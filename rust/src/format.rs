//! Canonical formatting that preserves source comments.

use crate::emit::{format_key, inline_value, is_block_string, push_indent, write_block};
use crate::parser::{FormatAnchor, FormatInfo, FullLineComment, InlineComment, parse_for_format};
use crate::span::pointer_child;
use crate::{Error, Map, Value};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

/// Formatting options loaded from a `.fmt.handlist` configuration file.
///
/// The default configuration enables no optional formatting rules. Parse a
/// configuration file with [`FormatConfig::parse`] and pass it to
/// [`crate::format_with_config`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormatConfig {
    allow_non_complex_one_line_seq: bool,
    allow_one_line_in_multi_line_text: bool,
}

impl FormatConfig {
    /// Parses a `.fmt.handlist` document.
    ///
    /// The document must be a mapping with `version: 1` and an optional `allow`
    /// sequence. Supported values are `non-complex-one-line-seq` and
    /// `one-line-in-multi-line-text`. Unknown keys, unknown allow values, and
    /// duplicates are errors.
    pub fn parse(source: &str) -> Result<Self, Error> {
        let value = crate::parse(source)?;
        let Value::Map(map) = value else {
            return Err(Error::new("format config must be a mapping"));
        };

        for key in map.keys() {
            if key != "version" && key != "allow" {
                return Err(Error::new(format!("unknown format config key {key:?}")));
            }
        }

        match map.get("version") {
            Some(Value::Int(1)) => {}
            Some(Value::Int(version)) => {
                return Err(Error::new(format!(
                    "unsupported format config version {version}; expected 1"
                )));
            }
            Some(_) => {
                return Err(Error::new("format config 'version' must be the integer 1"));
            }
            None => {
                return Err(Error::new("format config must contain 'version: 1'"));
            }
        }

        let mut config = Self::default();
        if let Some(allow) = map.get("allow") {
            let Value::Seq(items) = allow else {
                return Err(Error::new(
                    "format config 'allow' must be a sequence of option names",
                ));
            };
            for item in items {
                let Value::String(name) = item else {
                    return Err(Error::new("format config 'allow' entries must be strings"));
                };
                let slot = match name.as_str() {
                    "non-complex-one-line-seq" => &mut config.allow_non_complex_one_line_seq,
                    "one-line-in-multi-line-text" => &mut config.allow_one_line_in_multi_line_text,
                    other => {
                        return Err(Error::new(format!(
                            "unknown format config option {other:?}"
                        )));
                    }
                };
                if *slot {
                    return Err(Error::new(format!(
                        "duplicate format config option {name:?}"
                    )));
                }
                *slot = true;
            }
        }

        Ok(config)
    }

    /// Returns whether `non-complex-one-line-seq` is enabled.
    pub fn allow_non_complex_one_line_seq(&self) -> bool {
        self.allow_non_complex_one_line_seq
    }

    /// Returns whether `one-line-in-multi-line-text` is enabled.
    pub fn allow_one_line_in_multi_line_text(&self) -> bool {
        self.allow_one_line_in_multi_line_text
    }
}

impl FromStr for FormatConfig {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Self::parse(source)
    }
}

pub(crate) fn to_string(source: &str) -> Result<String, Error> {
    to_string_with_config(source, &FormatConfig::default())
}

pub(crate) fn to_string_with_config(source: &str, config: &FormatConfig) -> Result<String, Error> {
    let (value, info) = parse_for_format(source)?;
    let formatter = Formatter::new(info, config);
    let mut output = String::new();
    emit_root(&value, &mut output, &formatter)?;

    while output.ends_with("\n\n") {
        output.pop();
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

#[derive(Clone, Debug)]
struct CommentBlock {
    lines: Vec<String>,
    blank_before: bool,
    blank_after: bool,
}

#[derive(Default)]
struct Style {
    inline_sequences: HashSet<String>,
    block_texts: HashSet<String>,
}

struct Formatter<'a> {
    comments: Comments,
    config: &'a FormatConfig,
    style: Style,
}

impl<'a> Formatter<'a> {
    fn new(info: FormatInfo, config: &'a FormatConfig) -> Self {
        let (comments, style) = Comments::new(info);
        Self {
            comments,
            config,
            style,
        }
    }

    fn is_source_inline_sequence(&self, path: &str, seq: &[Value]) -> bool {
        !seq.is_empty()
            && self.config.allow_non_complex_one_line_seq
            && self.style.inline_sequences.contains(path)
            && seq
                .iter()
                .all(|value| !matches!(value, Value::Seq(_) | Value::Map(_)))
    }

    fn use_block_text(&self, path: &str, text: &str) -> bool {
        is_block_string(text)
            || (self.config.allow_one_line_in_multi_line_text
                && self.style.block_texts.contains(path)
                && !text.contains('\n'))
    }
}

#[derive(Default)]
struct Comments {
    leading: HashMap<String, Vec<CommentBlock>>,
    trailing: HashMap<String, Vec<CommentBlock>>,
    inline: HashMap<String, String>,
    blank_before: HashSet<String>,
}

impl Comments {
    fn new(info: FormatInfo) -> (Self, Style) {
        let FormatInfo {
            full_line_comments,
            inline_comments,
            mut anchors,
            blank_lines,
            inline_sequences,
            block_texts,
        } = info;

        let style = Style {
            inline_sequences: inline_sequences.into_iter().collect(),
            block_texts: block_texts.into_iter().collect(),
        };

        anchors.sort_by_key(|anchor| (anchor.line, anchor.indent, anchor.order));
        let blank_lines: HashSet<usize> = blank_lines.into_iter().collect();
        let mut comments = Self::default();

        for InlineComment { path, text } in inline_comments {
            comments.inline.insert(path, text);
        }

        let groups = group_comments(full_line_comments);
        for group in groups {
            let first_line = group.first().expect("comment group is non-empty").line;
            let last_line = group.last().expect("comment group is non-empty").line;
            let indent = group.first().expect("comment group is non-empty").indent;
            let block = CommentBlock {
                lines: group.into_iter().map(|comment| comment.text).collect(),
                blank_before: first_line > 1 && blank_lines.contains(&(first_line - 1)),
                blank_after: blank_lines.contains(&(last_line + 1)),
            };

            let next = anchors.iter().find(|anchor| anchor.line > last_line);
            let previous = previous_anchor(&anchors, first_line, indent);

            match (next, previous) {
                (Some(next), None) => {
                    comments
                        .leading
                        .entry(next.path.clone())
                        .or_default()
                        .push(block);
                }
                (Some(next), Some(_)) if next.indent >= indent => {
                    comments
                        .leading
                        .entry(next.path.clone())
                        .or_default()
                        .push(block);
                }
                (_, Some(previous)) => {
                    comments
                        .trailing
                        .entry(previous.path.clone())
                        .or_default()
                        .push(block);
                }
                (None, None) => {
                    comments
                        .trailing
                        .entry(String::new())
                        .or_default()
                        .push(block);
                }
            }
        }

        // Preserve a separator blank line immediately before a root-level node
        // when it is not already covered by a leading comment block.
        for anchor in anchors
            .iter()
            .filter(|anchor| anchor.indent == 0 && !anchor.path.is_empty() && anchor.line > 1)
        {
            if blank_lines.contains(&(anchor.line - 1))
                && !comments.leading.contains_key(&anchor.path)
            {
                comments.blank_before.insert(anchor.path.clone());
            }
        }

        (comments, style)
    }

    fn leading(&self, path: &str) -> &[CommentBlock] {
        self.leading.get(path).map(Vec::as_slice).unwrap_or(&[])
    }

    fn trailing(&self, path: &str) -> &[CommentBlock] {
        self.trailing.get(path).map(Vec::as_slice).unwrap_or(&[])
    }

    fn inline(&self, path: &str) -> Option<&str> {
        self.inline.get(path).map(String::as_str)
    }

    fn emit_leading(&self, path: &str, indent: usize, output: &mut String) {
        if self.blank_before.contains(path) {
            ensure_blank(output);
        }
        emit_blocks(self.leading(path), indent, output);
    }

    fn emit_trailing(&self, path: &str, indent: usize, output: &mut String) {
        emit_blocks(self.trailing(path), indent, output);
    }
}

fn group_comments(mut comments: Vec<FullLineComment>) -> Vec<Vec<FullLineComment>> {
    comments.sort_by_key(|comment| comment.line);
    let mut groups: Vec<Vec<FullLineComment>> = Vec::new();
    for comment in comments {
        let continues_group = groups
            .last()
            .and_then(|group| group.last())
            .is_some_and(|last| last.line + 1 == comment.line);
        if continues_group {
            groups.last_mut().expect("group exists").push(comment);
        } else {
            groups.push(vec![comment]);
        }
    }
    groups
}

fn previous_anchor(anchors: &[FormatAnchor], line: usize, indent: usize) -> Option<&FormatAnchor> {
    let before: Vec<&FormatAnchor> = anchors.iter().filter(|anchor| anchor.line < line).collect();

    before
        .iter()
        .rev()
        .find(|anchor| anchor.indent == indent)
        .copied()
        .or_else(|| {
            before
                .iter()
                .filter(|anchor| anchor.indent < indent)
                .max_by_key(|anchor| (anchor.indent, anchor.line, anchor.order))
                .copied()
        })
}

fn emit_blocks(blocks: &[CommentBlock], indent: usize, output: &mut String) {
    for block in blocks {
        if block.blank_before {
            ensure_blank(output);
        }
        for line in &block.lines {
            push_indent(output, indent);
            output.push_str(line);
            output.push('\n');
        }
        if block.blank_after {
            ensure_blank(output);
        }
    }
}

fn ensure_blank(output: &mut String) {
    if !output.is_empty() && !output.ends_with("\n\n") {
        output.push('\n');
    }
}

fn write_inline_comment(output: &mut String, formatter: &Formatter<'_>, path: &str) {
    if let Some(comment) = formatter.comments.inline(path) {
        output.push(' ');
        output.push_str(comment);
    }
}

fn emit_inline_as_standalone(
    output: &mut String,
    formatter: &Formatter<'_>,
    path: &str,
    indent: usize,
) {
    if let Some(comment) = formatter.comments.inline(path) {
        push_indent(output, indent);
        output.push_str(comment);
        output.push('\n');
    }
}

fn flow_sequence(seq: &[Value]) -> Result<String, Error> {
    let mut text = String::from("[");
    for (index, value) in seq.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(&inline_value(value)?);
    }
    text.push(']');
    Ok(text)
}

fn write_block_content(text: &str, indent: usize, output: &mut String) {
    if !text.is_empty() {
        write_block(text, indent, output);
    }
}

fn emit_root(value: &Value, output: &mut String, formatter: &Formatter<'_>) -> Result<(), Error> {
    formatter.comments.emit_leading("", 0, output);

    match value {
        Value::Map(map) if !map.is_empty() => {
            emit_map_entries(map, 0, "", output, formatter, None)?;
            emit_inline_as_standalone(output, formatter, "", 0);
        }
        Value::Seq(seq) if !seq.is_empty() && formatter.is_source_inline_sequence("", seq) => {
            output.push_str(&flow_sequence(seq)?);
            write_inline_comment(output, formatter, "");
            output.push('\n');
        }
        Value::Seq(seq) if !seq.is_empty() => {
            emit_seq(seq, 0, "", output, formatter, None)?;
            emit_inline_as_standalone(output, formatter, "", 0);
        }
        Value::String(text) if formatter.use_block_text("", text) => {
            output.push('|');
            write_inline_comment(output, formatter, "");
            output.push('\n');
            write_block_content(text, 2, output);
        }
        _ => {
            output.push_str(&inline_value(value)?);
            write_inline_comment(output, formatter, "");
            output.push('\n');
        }
    }

    formatter.comments.emit_trailing("", 0, output);
    Ok(())
}

fn emit_map_entries(
    map: &Map,
    indent: usize,
    path: &str,
    output: &mut String,
    formatter: &Formatter<'_>,
    first_prefix: Option<&str>,
) -> Result<(), Error> {
    for (index, (key, value)) in map.iter().enumerate() {
        let child = pointer_child(path, key);
        let prefix = if index == 0 { first_prefix } else { None };

        if let Some(prefix) = prefix {
            // A compact first entry was already prefixed by its caller, and
            // any leading comments for it were hoisted before that line.
            output.push_str(prefix);
        } else {
            formatter.comments.emit_leading(&child, indent, output);
            push_indent(output, indent);
        }

        output.push_str(&format_key(key)?);
        output.push(':');
        emit_value_after_colon(value, indent, &child, output, formatter)?;
        formatter.comments.emit_trailing(&child, indent, output);
    }
    Ok(())
}

fn emit_value_after_colon(
    value: &Value,
    indent: usize,
    path: &str,
    output: &mut String,
    formatter: &Formatter<'_>,
) -> Result<(), Error> {
    match value {
        Value::Map(map) if !map.is_empty() => {
            output.push('\n');
            emit_map_entries(map, indent + 2, path, output, formatter, None)?;
            emit_inline_as_standalone(output, formatter, path, indent + 2);
            Ok(())
        }
        Value::Seq(seq) if !seq.is_empty() && formatter.is_source_inline_sequence(path, seq) => {
            output.push(' ');
            output.push_str(&flow_sequence(seq)?);
            write_inline_comment(output, formatter, path);
            output.push('\n');
            Ok(())
        }
        Value::Seq(seq) if !seq.is_empty() => {
            output.push('\n');
            emit_seq(seq, indent + 2, path, output, formatter, None)?;
            emit_inline_as_standalone(output, formatter, path, indent + 2);
            Ok(())
        }
        Value::String(text) if formatter.use_block_text(path, text) => {
            output.push_str(" |");
            write_inline_comment(output, formatter, path);
            output.push('\n');
            write_block_content(text, indent + 2, output);
            Ok(())
        }
        _ => {
            output.push(' ');
            output.push_str(&inline_value(value)?);
            write_inline_comment(output, formatter, path);
            output.push('\n');
            Ok(())
        }
    }
}

fn emit_seq(
    seq: &[Value],
    indent: usize,
    path: &str,
    output: &mut String,
    formatter: &Formatter<'_>,
    first_prefix: Option<&str>,
) -> Result<(), Error> {
    for (index, item) in seq.iter().enumerate() {
        let child = pointer_child(path, &index.to_string());
        let prefix = if index == 0 { first_prefix } else { None };

        if let Some(prefix) = prefix {
            output.push_str(prefix);
        } else {
            formatter.comments.emit_leading(&child, indent, output);
            hoist_compact_comments(item, &child, indent, output, formatter);
            push_indent(output, indent);
        }

        output.push_str("- ");
        emit_item_value(item, indent, &child, output, formatter)?;
        formatter.comments.emit_trailing(&child, indent, output);
    }
    Ok(())
}

/// Comments before the first key/item of a compact sequence item live before
/// the compact line, because the grammar does not allow a comment between the
/// sequence marker and the compact value.
fn hoist_compact_comments(
    item: &Value,
    path: &str,
    indent: usize,
    output: &mut String,
    formatter: &Formatter<'_>,
) {
    match item {
        Value::Map(map) if !map.is_empty() => {
            let (key, _) = map.iter().next().expect("map is non-empty");
            let child = pointer_child(path, key);
            formatter.comments.emit_leading(&child, indent, output);
        }
        Value::Seq(seq) if !seq.is_empty() => {
            let child = pointer_child(path, "0");
            formatter.comments.emit_leading(&child, indent, output);
            hoist_compact_comments(&seq[0], &child, indent, output, formatter);
        }
        _ => {}
    }
}

fn emit_item_value(
    item: &Value,
    indent: usize,
    path: &str,
    output: &mut String,
    formatter: &Formatter<'_>,
) -> Result<(), Error> {
    match item {
        Value::Map(map) if !map.is_empty() => {
            emit_map_entries(map, indent + 2, path, output, formatter, Some(""))?;
            emit_inline_as_standalone(output, formatter, path, indent);
            Ok(())
        }
        Value::Seq(seq) if !seq.is_empty() && formatter.is_source_inline_sequence(path, seq) => {
            output.push_str(&flow_sequence(seq)?);
            write_inline_comment(output, formatter, path);
            output.push('\n');
            Ok(())
        }
        Value::Seq(seq) if !seq.is_empty() => {
            emit_seq(seq, indent + 2, path, output, formatter, Some(""))?;
            emit_inline_as_standalone(output, formatter, path, indent);
            Ok(())
        }
        Value::String(text) if formatter.use_block_text(path, text) => {
            output.push('|');
            write_inline_comment(output, formatter, path);
            output.push('\n');
            write_block_content(text, indent + 2, output);
            Ok(())
        }
        _ => {
            output.push_str(&inline_value(item)?);
            write_inline_comment(output, formatter, path);
            output.push('\n');
            Ok(())
        }
    }
}
