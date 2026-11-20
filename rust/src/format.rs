//! Canonical formatting that preserves source comments.

use crate::emit::{format_key, inline_value, is_block_string, push_indent, write_block};
use crate::parser::{FormatAnchor, FormatInfo, FullLineComment, InlineComment, parse_for_format};
use crate::span::pointer_child;
use crate::{Error, Map, Value};
use std::collections::{HashMap, HashSet};

/// Formats a handlist document into canonical form while retaining comments.
pub(crate) fn to_string(source: &str) -> Result<String, Error> {
    let (value, info) = parse_for_format(source)?;
    let comments = Comments::new(info);
    let mut output = String::new();
    emit_root(&value, &mut output, &comments)?;

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
struct Comments {
    leading: HashMap<String, Vec<CommentBlock>>,
    trailing: HashMap<String, Vec<CommentBlock>>,
    inline: HashMap<String, String>,
    blank_before: HashSet<String>,
}

impl Comments {
    fn new(info: FormatInfo) -> Self {
        let FormatInfo {
            full_line_comments,
            inline_comments,
            mut anchors,
            blank_lines,
        } = info;

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

        comments
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

fn write_inline_comment(output: &mut String, comments: &Comments, path: &str) {
    if let Some(comment) = comments.inline(path) {
        output.push(' ');
        output.push_str(comment);
    }
}

fn emit_inline_as_standalone(output: &mut String, comments: &Comments, path: &str, indent: usize) {
    if let Some(comment) = comments.inline(path) {
        push_indent(output, indent);
        output.push_str(comment);
        output.push('\n');
    }
}

fn emit_root(value: &Value, output: &mut String, comments: &Comments) -> Result<(), Error> {
    comments.emit_leading("", 0, output);

    match value {
        Value::Map(map) if !map.is_empty() => {
            emit_map_entries(map, 0, "", output, comments, None)?;
            emit_inline_as_standalone(output, comments, "", 0);
        }
        Value::Seq(seq) if !seq.is_empty() => {
            emit_seq(seq, 0, "", output, comments, None)?;
            emit_inline_as_standalone(output, comments, "", 0);
        }
        Value::String(text) if is_block_string(text) => {
            output.push('|');
            write_inline_comment(output, comments, "");
            output.push('\n');
            write_block(text, 2, output);
        }
        _ => {
            output.push_str(&inline_value(value)?);
            write_inline_comment(output, comments, "");
            output.push('\n');
        }
    }

    comments.emit_trailing("", 0, output);
    Ok(())
}

fn emit_map_entries(
    map: &Map,
    indent: usize,
    path: &str,
    output: &mut String,
    comments: &Comments,
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
            comments.emit_leading(&child, indent, output);
            push_indent(output, indent);
        }

        output.push_str(&format_key(key)?);
        output.push(':');
        emit_value_after_colon(value, indent, &child, output, comments)?;
        comments.emit_trailing(&child, indent, output);
    }
    Ok(())
}

fn emit_value_after_colon(
    value: &Value,
    indent: usize,
    path: &str,
    output: &mut String,
    comments: &Comments,
) -> Result<(), Error> {
    match value {
        Value::Map(map) if !map.is_empty() => {
            output.push('\n');
            emit_map_entries(map, indent + 2, path, output, comments, None)?;
            emit_inline_as_standalone(output, comments, path, indent + 2);
            Ok(())
        }
        Value::Seq(seq) if !seq.is_empty() => {
            output.push('\n');
            emit_seq(seq, indent + 2, path, output, comments, None)?;
            emit_inline_as_standalone(output, comments, path, indent + 2);
            Ok(())
        }
        Value::String(text) if is_block_string(text) => {
            output.push_str(" |");
            write_inline_comment(output, comments, path);
            output.push('\n');
            write_block(text, indent + 2, output);
            Ok(())
        }
        _ => {
            output.push(' ');
            output.push_str(&inline_value(value)?);
            write_inline_comment(output, comments, path);
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
    comments: &Comments,
    first_prefix: Option<&str>,
) -> Result<(), Error> {
    for (index, item) in seq.iter().enumerate() {
        let child = pointer_child(path, &index.to_string());
        let prefix = if index == 0 { first_prefix } else { None };

        if let Some(prefix) = prefix {
            output.push_str(prefix);
        } else {
            comments.emit_leading(&child, indent, output);
            hoist_compact_comments(item, &child, indent, output, comments);
            push_indent(output, indent);
        }

        output.push_str("- ");
        emit_item_value(item, indent, &child, output, comments)?;
        comments.emit_trailing(&child, indent, output);
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
    comments: &Comments,
) {
    match item {
        Value::Map(map) if !map.is_empty() => {
            let (key, _) = map.iter().next().expect("map is non-empty");
            let child = pointer_child(path, key);
            comments.emit_leading(&child, indent, output);
        }
        Value::Seq(seq) if !seq.is_empty() => {
            let child = pointer_child(path, "0");
            comments.emit_leading(&child, indent, output);
            hoist_compact_comments(&seq[0], &child, indent, output, comments);
        }
        _ => {}
    }
}

fn emit_item_value(
    item: &Value,
    indent: usize,
    path: &str,
    output: &mut String,
    comments: &Comments,
) -> Result<(), Error> {
    match item {
        Value::Map(map) if !map.is_empty() => {
            emit_map_entries(map, indent + 2, path, output, comments, Some(""))?;
            emit_inline_as_standalone(output, comments, path, indent);
            Ok(())
        }
        Value::Seq(seq) if !seq.is_empty() => {
            emit_seq(seq, indent + 2, path, output, comments, Some(""))?;
            emit_inline_as_standalone(output, comments, path, indent);
            Ok(())
        }
        Value::String(text) if is_block_string(text) => {
            output.push('|');
            write_inline_comment(output, comments, path);
            output.push('\n');
            write_block(text, indent + 2, output);
            Ok(())
        }
        _ => {
            output.push_str(&inline_value(item)?);
            write_inline_comment(output, comments, path);
            output.push('\n');
            Ok(())
        }
    }
}
