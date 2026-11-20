//! The handlist parser.
//!
//! The parser is line-oriented and deliberately strict: it accepts exactly the
//! language described in `spec/handlist-spec.md`.

use crate::span::{Span, SpanMap, pointer_child};
use crate::value::MAX_SAFE_INT;
use crate::{Error, Map, Value};

/// Parses a handlist document into a [`Value`].
pub fn parse(source: &str) -> Result<Value, Error> {
    let mut parser = Parser::new(source)?;
    parser.parse_document()
}

pub(crate) fn parse_with_spans(source: &str) -> Result<(Value, SpanMap), Error> {
    let mut parser = Parser::new(source)?;
    parser.spans = Some(SpanMap::new());
    let value = parser.parse_document()?;
    Ok((value, parser.spans.take().unwrap_or_default()))
}

struct Parser<'a> {
    lines: Vec<Line<'a>>,
    pos: usize,
    spans: Option<SpanMap>,
}

#[derive(Clone, Copy)]
struct Line<'a> {
    number: usize,
    raw: &'a str,
    indent: usize,
    content: &'a str,
}

impl<'a> Line<'a> {
    fn is_blank(&self) -> bool {
        self.content.is_empty()
    }

    fn is_comment(&self) -> bool {
        self.content.starts_with('#')
    }

    fn trimmed_content(&self) -> &'a str {
        self.content.trim_end_matches(' ')
    }
}

struct InlineEntry<'a> {
    text: &'a str,
    line: usize,
    column: usize,
}

struct InlineItem<'a> {
    text: &'a str,
    line: usize,
    column: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self, Error> {
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        validate_source(source)?;

        let mut lines = Vec::new();
        for (index, part) in source.split('\n').enumerate() {
            let raw = part.strip_suffix('\r').unwrap_or(part);
            let indent = raw.bytes().take_while(|byte| *byte == b' ').count();
            lines.push(Line {
                number: index + 1,
                raw,
                indent,
                content: &raw[indent..],
            });
        }

        Ok(Self {
            lines,
            pos: 0,
            spans: None,
        })
    }

    fn snapshot(&self) -> (usize, usize, &'a str) {
        let line = &self.lines[self.pos];
        (line.number, line.indent, line.trimmed_content())
    }

    fn skip_trivia(&mut self) {
        while self.pos < self.lines.len() {
            let line = &self.lines[self.pos];
            if line.is_blank() || line.is_comment() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn child_pointer(&self, parent: &str, token: &str) -> String {
        if self.spans.is_some() {
            pointer_child(parent, token)
        } else {
            String::new()
        }
    }

    fn record_value(&mut self, pointer: &str, line: usize, column: usize) {
        if let Some(spans) = &mut self.spans {
            spans.insert_value(pointer, Span::new(line, column));
        }
    }

    fn record_key(&mut self, pointer: &str, line: usize, column: usize) {
        if let Some(spans) = &mut self.spans {
            spans.insert_key(pointer, Span::new(line, column));
        }
    }

    fn error_eof(&self, message: &str) -> Error {
        match self.lines.last() {
            Some(line) => Error::at(message, line.number, line.raw.chars().count() + 1),
            None => Error::at(message, 1, 1),
        }
    }

    fn parse_document(&mut self) -> Result<Value, Error> {
        self.skip_trivia();
        if self.pos >= self.lines.len() {
            return Err(self.error_eof("a handlist document must contain a root node"));
        }

        let (number, indent, content) = self.snapshot();
        if indent != 0 {
            return Err(Error::at(
                "the root node must start at column 1",
                number,
                indent + 1,
            ));
        }
        if is_document_marker(content) {
            return Err(Error::at(
                "YAML document markers ('---' and '...') are not supported",
                number,
                1,
            ));
        }

        self.record_value("", number, 1);

        let value = if is_sequence_start(content) {
            self.parse_sequence(0, None, "")?
        } else if split_key_prefix(content, number, 1)?.is_some() {
            self.parse_mapping(0, None, "")?
        } else {
            self.parse_root_scalar(number, content, "")?
        };

        self.skip_trivia();
        if self.pos < self.lines.len() {
            let (number, indent, _) = self.snapshot();
            return Err(Error::at(
                "unexpected content after the root node",
                number,
                indent + 1,
            ));
        }

        Ok(value)
    }

    fn parse_root_scalar(
        &mut self,
        line: usize,
        text: &'a str,
        pointer: &str,
    ) -> Result<Value, Error> {
        self.pos += 1;
        let (value, end) = self.parse_inline_node(text, line, 1, 0, pointer)?;
        self.finish_inline(text, end, line, 1)?;
        Ok(value)
    }

    fn parse_mapping(
        &mut self,
        indent: usize,
        first: Option<InlineEntry<'a>>,
        pointer: &str,
    ) -> Result<Value, Error> {
        let mut map = Map::new();

        if let Some(entry) = first {
            let (key, value, column) =
                self.parse_entry_fragment(indent, entry.text, entry.line, entry.column, pointer)?;
            insert_entry(&mut map, key, value, entry.line, column)?;
        }

        loop {
            self.skip_trivia();
            if self.pos >= self.lines.len() {
                break;
            }

            let (number, actual, content) = self.snapshot();
            if actual < indent {
                break;
            }
            if actual > indent {
                return Err(Error::at(
                    format!("unexpected indentation: expected {indent} spaces for this mapping"),
                    number,
                    actual + 1,
                ));
            }
            if content.is_empty() {
                self.pos += 1;
                continue;
            }
            if is_sequence_start(content) {
                return Err(Error::at(
                    "expected a mapping key, found a sequence item",
                    number,
                    actual + 1,
                ));
            }

            let Some((key, after_colon)) = split_key_prefix(content, number, indent + 1)? else {
                return Err(Error::at(
                    "expected a mapping key such as 'name:'",
                    number,
                    indent + 1,
                ));
            };

            self.pos += 1;
            let child_pointer = self.child_pointer(pointer, &key);
            self.record_key(&child_pointer, number, indent + 1);
            let value = self.parse_entry_value(
                indent,
                content,
                after_colon,
                number,
                indent + 1,
                &child_pointer,
            )?;
            insert_entry(&mut map, key, value, number, indent + 1)?;
        }

        Ok(Value::Map(map))
    }

    fn parse_entry_fragment(
        &mut self,
        mapping_indent: usize,
        text: &'a str,
        line: usize,
        column: usize,
        pointer: &str,
    ) -> Result<(String, Value, usize), Error> {
        let Some((key, after_colon)) = split_key_prefix(text, line, column)? else {
            return Err(Error::at("expected a mapping key", line, column));
        };
        let child_pointer = self.child_pointer(pointer, &key);
        self.record_key(&child_pointer, line, column);
        let value = self.parse_entry_value(
            mapping_indent,
            text,
            after_colon,
            line,
            column,
            &child_pointer,
        )?;
        Ok((key, value, column))
    }

    fn parse_entry_value(
        &mut self,
        mapping_indent: usize,
        text: &'a str,
        after_colon: usize,
        line: usize,
        base_column: usize,
        pointer: &str,
    ) -> Result<Value, Error> {
        let rest = &text[after_colon..];
        if rest.trim_end_matches(' ').is_empty() {
            return self.parse_child_block(mapping_indent + 2, pointer);
        }

        let spaces = rest.len() - rest.trim_start_matches(' ').len();
        if spaces == 0 {
            return Err(Error::at(
                "expected a space between ':' and the value",
                line,
                base_column + text[..after_colon].chars().count(),
            ));
        }

        let value_text = &rest[spaces..];
        let value_column = base_column + text[..after_colon + spaces].chars().count();
        if value_text.starts_with('#') {
            return Err(Error::at(
                "mapping value is missing before the comment",
                line,
                value_column,
            ));
        }
        if value_text.starts_with('\t') {
            return Err(Error::at(
                "tabs are not allowed between ':' and the value",
                line,
                value_column,
            ));
        }

        let (value, end) =
            self.parse_inline_node(value_text, line, value_column, mapping_indent, pointer)?;
        self.finish_inline(value_text, end, line, value_column)?;
        Ok(value)
    }

    fn parse_child_block(&mut self, indent: usize, pointer: &str) -> Result<Value, Error> {
        self.skip_trivia();
        if self.pos >= self.lines.len() {
            return Err(self.error_eof("expected a child block"));
        }

        let (number, actual, content) = self.snapshot();
        if actual > indent {
            return Err(Error::at(
                format!(
                    "unexpected indentation: child blocks must be indented exactly {indent} spaces"
                ),
                number,
                actual + 1,
            ));
        }
        if actual < indent {
            return Err(Error::at(
                format!("expected a child block indented {indent} spaces"),
                number,
                actual + 1,
            ));
        }

        self.record_value(pointer, number, indent + 1);

        if is_sequence_start(content) {
            self.parse_sequence(indent, None, pointer)
        } else if split_key_prefix(content, number, indent + 1)?.is_some() {
            self.parse_mapping(indent, None, pointer)
        } else {
            Err(Error::at(
                "a value on a new line must be a mapping or a sequence; write scalars on the key line",
                number,
                indent + 1,
            ))
        }
    }

    fn parse_sequence(
        &mut self,
        indent: usize,
        first: Option<InlineItem<'a>>,
        pointer: &str,
    ) -> Result<Value, Error> {
        let mut items = Vec::new();
        let mut item_index = 0usize;

        if let Some(item) = first {
            let child_pointer = self.child_pointer(pointer, &item_index.to_string());
            items.push(self.parse_sequence_item(
                indent,
                item.text,
                item.line,
                item.column,
                &child_pointer,
            )?);
            item_index += 1;
        }

        loop {
            self.skip_trivia();
            if self.pos >= self.lines.len() {
                break;
            }

            let (number, actual, content) = self.snapshot();
            if actual < indent {
                break;
            }
            if actual > indent {
                return Err(Error::at(
                    format!("unexpected indentation: expected {indent} spaces for this sequence"),
                    number,
                    actual + 1,
                ));
            }
            if !is_sequence_start(content) {
                return Err(Error::at(
                    "expected a sequence item ('- ')",
                    number,
                    actual + 1,
                ));
            }

            self.pos += 1;
            let child_pointer = self.child_pointer(pointer, &item_index.to_string());
            items.push(self.parse_sequence_item(
                indent,
                content,
                number,
                indent + 1,
                &child_pointer,
            )?);
            item_index += 1;
        }

        Ok(Value::Seq(items))
    }

    fn parse_sequence_item(
        &mut self,
        indent: usize,
        text: &'a str,
        line: usize,
        column: usize,
        pointer: &str,
    ) -> Result<Value, Error> {
        if text == "-" {
            return Err(Error::at("a sequence item must have a value", line, column));
        }

        let Some(after) = text.strip_prefix("- ") else {
            return Err(Error::at(
                "a sequence item marker must be '-' followed by one space",
                line,
                column,
            ));
        };

        if after.is_empty() {
            return Err(Error::at(
                "a sequence item must have a value",
                line,
                column + 2,
            ));
        }
        if after.starts_with(' ') {
            return Err(Error::at(
                "exactly one space is allowed after '-'",
                line,
                column + 2,
            ));
        }

        self.record_value(pointer, line, column + 2);
        self.parse_item_value(indent, after, line, column + 2, pointer)
    }

    fn parse_item_value(
        &mut self,
        indent: usize,
        text: &'a str,
        line: usize,
        column: usize,
        pointer: &str,
    ) -> Result<Value, Error> {
        if text == "-" || text.starts_with("- ") {
            return self.parse_sequence(
                indent + 2,
                Some(InlineItem { text, line, column }),
                pointer,
            );
        }

        if split_key_prefix(text, line, column)?.is_some() {
            return self.parse_mapping(
                indent + 2,
                Some(InlineEntry { text, line, column }),
                pointer,
            );
        }

        if text.starts_with('#') {
            return Err(Error::at(
                "sequence item value is missing before the comment",
                line,
                column,
            ));
        }

        let (value, end) = self.parse_inline_node(text, line, column, indent, pointer)?;
        self.finish_inline(text, end, line, column)?;
        Ok(value)
    }

    fn parse_inline_node(
        &mut self,
        text: &'a str,
        line: usize,
        column: usize,
        entry_indent: usize,
        pointer: &str,
    ) -> Result<(Value, usize), Error> {
        let Some(first) = text.chars().next() else {
            return Err(Error::at("expected a value", line, column));
        };

        self.record_value(pointer, line, column);

        match first {
            '|' => {
                let value = self.parse_block_text(text, line, column, entry_indent)?;
                Ok((Value::String(value), text.len()))
            }
            '>' => Err(Error::at(
                "folded block text ('>') is not supported; use '|'",
                line,
                column,
            )),
            '"' => {
                let (value, end) = scan_quoted(text, line, column)?;
                Ok((Value::String(value), end))
            }
            '[' => self.parse_flow_sequence(text, line, column, pointer),
            '{' => self.parse_empty_mapping(text, line, column),
            '\'' => Err(Error::at(
                "single-quoted strings are not supported; use double quotes",
                line,
                column,
            )),
            '#' => Err(Error::at(
                "expected a value before the comment",
                line,
                column,
            )),
            _ => {
                if first == '-' && (text.len() == 1 || text.as_bytes().get(1) == Some(&b' ')) {
                    return Err(Error::at(
                        "an inline node cannot start with '- '; use a block sequence",
                        line,
                        column,
                    ));
                }
                self.parse_plain(text, line, column)
            }
        }
    }

    fn parse_plain(
        &mut self,
        text: &'a str,
        line: usize,
        column: usize,
    ) -> Result<(Value, usize), Error> {
        let mut end = text.len();

        for (index, ch) in text.char_indices() {
            if ch == '#' && index > 0 && text.as_bytes()[index - 1] == b' ' {
                let next = text[index + 1..].chars().next();
                if next.is_none() || next == Some(' ') {
                    end = index - 1;
                    break;
                }
                let hash_column = column + text[..index].chars().count();
                return Err(Error::at(
                    "an inline comment must be written as '# ' (hash followed by a space) or end with '#' at the end of the line",
                    line,
                    hash_column,
                ));
            }
        }

        let value_text = text[..end].trim_end_matches(' ');
        if value_text.is_empty() {
            return Err(Error::at("expected a value", line, column));
        }

        validate_plain(value_text, line, column, false)?;
        let value = resolve_plain(value_text, line, column)?;
        Ok((value, end))
    }

    fn parse_flow_sequence(
        &mut self,
        text: &'a str,
        line: usize,
        column: usize,
        pointer: &str,
    ) -> Result<(Value, usize), Error> {
        let mut values = Vec::new();
        let mut element_index = 0usize;
        let mut index = skip_flow_spaces(text, 1, line, column)?;

        if index >= text.len() {
            return Err(Error::at(
                "unterminated flow sequence",
                line,
                column + text.chars().count(),
            ));
        }
        if text[index..].starts_with(']') {
            return Ok((Value::Seq(values), index + 1));
        }

        loop {
            if index >= text.len() {
                return Err(Error::at(
                    "unterminated flow sequence",
                    line,
                    column + text.chars().count(),
                ));
            }

            let element_column = column + text[..index].chars().count();
            let first = text[index..].chars().next().expect("index is in bounds");

            if first == ']' || first == ',' {
                return Err(Error::at(
                    "empty flow-sequence element",
                    line,
                    element_column,
                ));
            }
            if first == '#' {
                return Err(Error::at(
                    "comments are not allowed inside a flow sequence",
                    line,
                    element_column,
                ));
            }
            if first == '[' || first == '{' {
                return Err(Error::at(
                    "nested flow collections are not supported inside a flow sequence",
                    line,
                    element_column,
                ));
            }

            let element_pointer = self.child_pointer(pointer, &element_index.to_string());
            self.record_value(&element_pointer, line, element_column);
            element_index += 1;

            if first == '"' {
                let (value, consumed) = scan_quoted(&text[index..], line, element_column)?;
                values.push(Value::String(value));
                index += consumed;
            } else {
                let mut end = text.len();
                let mut terminated = false;
                for (offset, ch) in text[index..].char_indices() {
                    if ch == ',' || ch == ']' {
                        end = index + offset;
                        terminated = true;
                        break;
                    }
                }
                if !terminated {
                    return Err(Error::at(
                        "a flow sequence must be closed on the same line",
                        line,
                        element_column,
                    ));
                }

                let element = text[index..end].trim_end_matches(' ');
                if element.is_empty() {
                    return Err(Error::at(
                        "empty flow-sequence element",
                        line,
                        element_column,
                    ));
                }
                validate_plain(element, line, element_column, true)?;
                values.push(resolve_plain(element, line, element_column)?);
                index = end;
            }

            index = skip_flow_spaces(text, index, line, column)?;
            if index >= text.len() {
                return Err(Error::at(
                    "unterminated flow sequence",
                    line,
                    column + text.chars().count(),
                ));
            }

            match text[index..].chars().next().expect("index is in bounds") {
                ',' => {
                    index += 1;
                    index = skip_flow_spaces(text, index, line, column)?;
                    if index >= text.len() {
                        return Err(Error::at(
                            "unterminated flow sequence",
                            line,
                            column + text.chars().count(),
                        ));
                    }
                    let next = text[index..].chars().next().expect("index is in bounds");
                    if next == ',' || next == ']' {
                        return Err(Error::at(
                            "empty flow-sequence element",
                            line,
                            column + text[..index].chars().count(),
                        ));
                    }
                }
                ']' => return Ok((Value::Seq(values), index + 1)),
                other => {
                    return Err(Error::at(
                        format!("expected ',' or ']' in a flow sequence, found '{other}'"),
                        line,
                        column + text[..index].chars().count(),
                    ));
                }
            }
        }
    }

    fn parse_empty_mapping(
        &mut self,
        text: &'a str,
        line: usize,
        column: usize,
    ) -> Result<(Value, usize), Error> {
        let index = skip_flow_spaces(text, 1, line, column)?;
        if index >= text.len() || !text[index..].starts_with('}') {
            return Err(Error::at(
                "only the empty flow mapping '{}' is supported",
                line,
                column,
            ));
        }
        Ok((Value::Map(Map::new()), index + 1))
    }

    fn parse_block_text(
        &mut self,
        text: &'a str,
        line: usize,
        column: usize,
        entry_indent: usize,
    ) -> Result<String, Error> {
        let rest = &text[1..];
        if !rest.is_empty() {
            let mut index = 0usize;
            while index < rest.len() && rest[index..].starts_with(' ') {
                index += 1;
            }
            if index < rest.len() {
                if rest[index..].starts_with('#') {
                    if index == 0 {
                        return Err(Error::at(
                            "an inline comment must be separated from '|' by a space",
                            line,
                            column + 1,
                        ));
                    }
                    let after_hash = &rest[index + 1..];
                    if !after_hash.is_empty() && !after_hash.starts_with(' ') {
                        return Err(Error::at(
                            "an inline comment must be written as '# ' (hash followed by a space) or end with '#' at the end of the line",
                            line,
                            column + 1 + index,
                        ));
                    }
                } else {
                    return Err(Error::at(
                        "the block text marker must be exactly '|'; '>', '|-', '|+' and indentation indicators are not supported",
                        line,
                        column,
                    ));
                }
            }
        }

        let base = entry_indent + 2;
        let mut content_lines: Vec<String> = Vec::new();

        while self.pos < self.lines.len() {
            let raw = self.lines[self.pos].raw;
            let indent = self.lines[self.pos].indent;
            let line_number = self.lines[self.pos].number;

            if raw.chars().all(|ch| ch == ' ') {
                content_lines.push(String::new());
                self.pos += 1;
                continue;
            }

            if indent < base {
                if indent > entry_indent {
                    return Err(Error::at(
                        format!("block text content must be indented {base} spaces"),
                        line_number,
                        indent + 1,
                    ));
                }
                break;
            }

            let content = &raw[base..];
            content_lines.push(content.trim_end_matches([' ', '\t']).to_owned());
            self.pos += 1;
        }

        while content_lines.first().is_some_and(String::is_empty) {
            content_lines.remove(0);
        }
        while content_lines.last().is_some_and(String::is_empty) {
            content_lines.pop();
        }

        Ok(content_lines.join("\n"))
    }

    fn finish_inline(
        &self,
        text: &str,
        end: usize,
        line: usize,
        column: usize,
    ) -> Result<(), Error> {
        let rest = &text[end..];
        let mut index = 0usize;
        let mut spaces = 0usize;
        while index < rest.len() && rest[index..].starts_with(' ') {
            index += 1;
            spaces += 1;
        }
        if index == rest.len() {
            return Ok(());
        }

        let ch = rest[index..].chars().next().expect("index is in bounds");
        let error_column = column + text[..end + index].chars().count();
        if ch != '#' {
            return Err(Error::at(
                format!("unexpected character '{ch}' after the value"),
                line,
                error_column,
            ));
        }
        if spaces == 0 {
            return Err(Error::at(
                "an inline comment must be preceded by a space",
                line,
                error_column,
            ));
        }
        let after_hash = &rest[index + 1..];
        if !after_hash.is_empty() && !after_hash.starts_with(' ') {
            return Err(Error::at(
                "an inline comment must be written as '# ' (hash followed by a space) or end with '#' at the end of the line",
                line,
                error_column + 1,
            ));
        }
        Ok(())
    }
}

fn insert_entry(
    map: &mut Map,
    key: String,
    value: Value,
    line: usize,
    column: usize,
) -> Result<(), Error> {
    if map.contains_key(&key) {
        return Err(Error::at(
            format!("duplicate mapping key {key:?}"),
            line,
            column,
        ));
    }
    map.insert(key, value);
    Ok(())
}

fn validate_source(source: &str) -> Result<(), Error> {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\n' => {
                line += 1;
                column = 1;
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    line += 1;
                    column = 1;
                } else {
                    return Err(Error::at(
                        "a bare carriage return is not allowed; use LF or CRLF",
                        line,
                        column,
                    ));
                }
            }
            '\t' => column += 1,
            '\u{feff}' => {
                return Err(Error::at(
                    "a byte-order mark is only allowed at the very start of the file",
                    line,
                    column,
                ));
            }
            ch if ch.is_control() => {
                return Err(Error::at(
                    format!("control character U+{:04X} is not allowed", ch as u32),
                    line,
                    column,
                ));
            }
            '\u{fffe}' | '\u{ffff}' => {
                return Err(Error::at(
                    format!("noncharacter U+{:04X} is not allowed", ch as u32),
                    line,
                    column,
                ));
            }
            _ => column += 1,
        }
    }

    Ok(())
}

fn is_document_marker(content: &str) -> bool {
    ["---", "..."]
        .iter()
        .any(|marker| match content.strip_prefix(marker) {
            Some(rest) => rest.is_empty() || rest.starts_with(' '),
            None => false,
        })
}

fn is_sequence_start(content: &str) -> bool {
    content == "-" || content.starts_with("- ")
}

fn is_bare_key_start(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_bare_key_char(ch: char) -> bool {
    is_bare_key_start(ch) || ch == '-'
}

fn split_key_prefix(
    text: &str,
    line: usize,
    column: usize,
) -> Result<Option<(String, usize)>, Error> {
    if text.starts_with('"') {
        let (key, end) = scan_quoted(text, line, column)?;
        if !text[end..].starts_with(':') || !colon_is_separator(text, end) {
            return Ok(None);
        }
        validate_key(&key, line, column)?;
        return Ok(Some((key, end + 1)));
    }

    let Some(first) = text.chars().next() else {
        return Ok(None);
    };
    if !is_bare_key_start(first) {
        return Ok(None);
    }

    let mut end = first.len_utf8();
    for ch in text[end..].chars() {
        if is_bare_key_char(ch) {
            end += ch.len_utf8();
        } else {
            break;
        }
    }

    if !text[end..].starts_with(':') || !colon_is_separator(text, end) {
        return Ok(None);
    }

    Ok(Some((text[..end].to_owned(), end + 1)))
}

fn colon_is_separator(text: &str, colon: usize) -> bool {
    let after = &text[colon + 1..];
    after.is_empty() || after.starts_with(' ')
}

fn validate_key(key: &str, line: usize, column: usize) -> Result<(), Error> {
    if key.is_empty() {
        return Err(Error::at("mapping keys must not be empty", line, column));
    }
    if let Some(ch) = key.chars().find(|ch| ch.is_control()) {
        return Err(Error::at(
            format!(
                "mapping keys must not contain control characters (found U+{:04X})",
                ch as u32
            ),
            line,
            column,
        ));
    }
    Ok(())
}

fn scan_quoted(text: &str, line: usize, column: usize) -> Result<(String, usize), Error> {
    let mut value = String::new();
    let mut index = 1usize;

    while index < text.len() {
        let ch = text[index..].chars().next().expect("index is in bounds");
        let char_column = column + text[..index].chars().count();

        match ch {
            '"' => return Ok((value, index + 1)),
            '\\' => {
                let Some(next) = text[index + 1..].chars().next() else {
                    return Err(Error::at("unterminated escape sequence", line, char_column));
                };
                let escaped = match next {
                    '"' => '"',
                    '\\' => '\\',
                    'n' => '\n',
                    't' => '\t',
                    'u' | 'U' | 'x' => {
                        return Err(Error::at(
                            "Unicode and hex escapes are not supported; write the character directly as UTF-8",
                            line,
                            char_column,
                        ));
                    }
                    other => {
                        return Err(Error::at(
                            format!("unsupported escape sequence '\\{other}'"),
                            line,
                            char_column,
                        ));
                    }
                };
                value.push(escaped);
                index += 1 + next.len_utf8();
            }
            '\t' => {
                return Err(Error::at(
                    "raw tabs must be written as '\\t' inside a double-quoted string",
                    line,
                    char_column,
                ));
            }
            ch if ch.is_control() => {
                return Err(Error::at(
                    format!("control character U+{:04X} is not allowed", ch as u32),
                    line,
                    char_column,
                ));
            }
            _ => {
                value.push(ch);
                index += ch.len_utf8();
            }
        }
    }

    Err(Error::at(
        "unterminated double-quoted string",
        line,
        column + text.chars().count(),
    ))
}

fn validate_plain(value: &str, line: usize, column: usize, flow: bool) -> Result<(), Error> {
    let Some(first) = value.chars().next() else {
        return Err(Error::at("expected a value", line, column));
    };

    if first == '\t' {
        return Err(Error::at(
            "tabs are not allowed in plain scalars",
            line,
            column,
        ));
    }
    if is_forbidden_plain_start(first) || first == '?' || first == ':' {
        return Err(Error::at(
            format!("a plain scalar cannot start with '{first}'"),
            line,
            column,
        ));
    }
    if first == '-' && (value.len() == 1 || value.as_bytes().get(1) == Some(&b' ')) {
        return Err(Error::at(
            "a plain scalar cannot start with '- '",
            line,
            column,
        ));
    }

    for (index, ch) in value.char_indices() {
        let char_column = column + value[..index].chars().count();

        if ch == '\t' {
            return Err(Error::at(
                "tabs are not allowed in plain scalars",
                line,
                char_column,
            ));
        }
        if ch.is_control() {
            return Err(Error::at(
                format!("control character U+{:04X} is not allowed", ch as u32),
                line,
                char_column,
            ));
        }
        if ch == ':' {
            let next = value[index + 1..].chars().next();
            if next.is_none() || next == Some(' ') {
                return Err(Error::at(
                    "a colon in a plain scalar must not be followed by a space or end the line; quote the value",
                    line,
                    char_column,
                ));
            }
        }
        if flow && matches!(ch, ',' | '[' | ']' | '{' | '}') {
            return Err(Error::at(
                format!("'{ch}' is not allowed in a flow-sequence element"),
                line,
                char_column,
            ));
        }
        if ch == '#' && index > 0 && value.as_bytes()[index - 1] == b' ' {
            return Err(Error::at(
                "a plain scalar cannot contain a space followed by '#'; quote the value",
                line,
                char_column,
            ));
        }
    }

    Ok(())
}

fn is_forbidden_plain_start(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '\t'
            | '#'
            | '"'
            | '\''
            | '['
            | ']'
            | '{'
            | '}'
            | ','
            | '&'
            | '*'
            | '!'
            | '|'
            | '>'
            | '%'
            | '@'
            | '`'
    )
}

fn resolve_plain(value: &str, line: usize, column: usize) -> Result<Value, Error> {
    match value {
        "null" => return Ok(Value::Null),
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }

    if is_int_syntax(value) {
        let parsed = value
            .parse::<i64>()
            .map_err(|_| Error::at(format!("integer {value:?} is out of range"), line, column))?;
        if !(-MAX_SAFE_INT..=MAX_SAFE_INT).contains(&parsed) {
            return Err(Error::at(
                format!("integer {value:?} is outside the safe integer range"),
                line,
                column,
            ));
        }
        return Ok(Value::Int(parsed));
    }

    if is_float_syntax(value) {
        let parsed = value
            .parse::<f64>()
            .map_err(|_| Error::at(format!("invalid float {value:?}"), line, column))?;
        if !parsed.is_finite() {
            return Err(Error::at(
                format!("float {value:?} is out of range"),
                line,
                column,
            ));
        }
        return Ok(Value::Float(parsed));
    }

    Ok(Value::String(value.to_owned()))
}

pub(crate) fn is_int_syntax(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() {
        return false;
    }
    if digits == "0" {
        return true;
    }
    if digits.starts_with('0') {
        return false;
    }
    digits.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn is_float_syntax(value: &str) -> bool {
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let Some((integer, fraction)) = unsigned.split_once('.') else {
        return false;
    };
    if integer.is_empty() || fraction.is_empty() {
        return false;
    }
    if integer != "0" && integer.starts_with('0') {
        return false;
    }
    integer.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

fn skip_flow_spaces(
    text: &str,
    mut index: usize,
    line: usize,
    column: usize,
) -> Result<usize, Error> {
    while index < text.len() {
        let ch = text[index..].chars().next().expect("index is in bounds");
        if ch == ' ' {
            index += 1;
        } else if ch == '\t' {
            return Err(Error::at(
                "tabs are not allowed inside a flow sequence",
                line,
                column + text[..index].chars().count(),
            ));
        } else {
            break;
        }
    }
    Ok(index)
}
