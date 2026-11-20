//! Canonical handlist serialization.

use crate::parser::{is_float_syntax, is_int_syntax};
use crate::value::MAX_SAFE_INT;
use crate::{Error, Map, Value};

/// Serializes a [`Value`] into canonical handlist text.
///
/// The output always ends with a single LF and always parses back to an equal
/// value.
pub fn to_string(value: &Value) -> Result<String, Error> {
    let mut output = String::new();
    emit_root(value, &mut output)?;
    Ok(output)
}

fn emit_root(value: &Value, output: &mut String) -> Result<(), Error> {
    match value {
        Value::Map(map) if !map.is_empty() => emit_map_entries(map, 0, output, None),
        Value::Seq(seq) if !seq.is_empty() => emit_seq(seq, 0, output, None),
        Value::String(text) if is_block_string(text) => {
            output.push_str("|\n");
            write_block(text, 2, output);
            Ok(())
        }
        _ => {
            output.push_str(&inline_value(value)?);
            output.push('\n');
            Ok(())
        }
    }
}

fn emit_map_entries(
    map: &Map,
    indent: usize,
    output: &mut String,
    first_prefix: Option<&str>,
) -> Result<(), Error> {
    for (index, (key, value)) in map.iter().enumerate() {
        if index == 0 {
            if let Some(prefix) = first_prefix {
                output.push_str(prefix);
            } else {
                push_indent(output, indent);
            }
        } else {
            push_indent(output, indent);
        }

        output.push_str(&format_key(key)?);
        output.push(':');
        emit_value_after_colon(value, indent, output)?;
    }
    Ok(())
}

fn emit_value_after_colon(value: &Value, indent: usize, output: &mut String) -> Result<(), Error> {
    match value {
        Value::Map(map) if !map.is_empty() => {
            output.push('\n');
            emit_map_entries(map, indent + 2, output, None)
        }
        Value::Seq(seq) if !seq.is_empty() => {
            output.push('\n');
            emit_seq(seq, indent + 2, output, None)
        }
        Value::String(text) if is_block_string(text) => {
            output.push_str(" |\n");
            write_block(text, indent + 2, output);
            Ok(())
        }
        _ => {
            output.push(' ');
            output.push_str(&inline_value(value)?);
            output.push('\n');
            Ok(())
        }
    }
}

fn emit_seq(
    seq: &[Value],
    indent: usize,
    output: &mut String,
    first_prefix: Option<&str>,
) -> Result<(), Error> {
    for (index, item) in seq.iter().enumerate() {
        if index == 0 {
            if let Some(prefix) = first_prefix {
                output.push_str(prefix);
            } else {
                push_indent(output, indent);
            }
        } else {
            push_indent(output, indent);
        }

        output.push_str("- ");
        emit_item_value(item, indent, output)?;
    }
    Ok(())
}

fn emit_item_value(item: &Value, indent: usize, output: &mut String) -> Result<(), Error> {
    match item {
        Value::Map(map) if !map.is_empty() => emit_map_entries(map, indent + 2, output, Some("")),
        Value::Seq(seq) if !seq.is_empty() => emit_seq(seq, indent + 2, output, Some("")),
        Value::String(text) if is_block_string(text) => {
            output.push_str("|\n");
            write_block(text, indent + 2, output);
            Ok(())
        }
        _ => {
            output.push_str(&inline_value(item)?);
            output.push('\n');
            Ok(())
        }
    }
}

pub(crate) fn inline_value(value: &Value) -> Result<String, Error> {
    match value {
        Value::Null => Ok("null".to_owned()),
        Value::Bool(true) => Ok("true".to_owned()),
        Value::Bool(false) => Ok("false".to_owned()),
        Value::Int(integer) => {
            if *integer < -MAX_SAFE_INT || *integer > MAX_SAFE_INT {
                return Err(Error::new(format!(
                    "integer {integer} is outside the safe integer range"
                )));
            }
            Ok(integer.to_string())
        }
        Value::Float(float) => format_float(*float),
        Value::String(text) => {
            if plain_safe(text) {
                Ok(text.clone())
            } else {
                quoted(text)
            }
        }
        Value::Seq(seq) if seq.is_empty() => Ok("[]".to_owned()),
        Value::Map(map) if map.is_empty() => Ok("{}".to_owned()),
        other => Err(Error::new(format!(
            "cannot inline a non-empty {}",
            other.kind()
        ))),
    }
}

pub(crate) fn format_key(key: &str) -> Result<String, Error> {
    if key.is_empty() {
        return Err(Error::new("mapping keys must not be empty"));
    }
    if key.chars().any(char::is_control)
        || key.contains('\u{feff}')
        || key.contains('\u{fffe}')
        || key.contains('\u{ffff}')
    {
        return Err(Error::new(
            "mapping keys containing control characters or noncharacters cannot be represented",
        ));
    }
    if is_bare_key(key) {
        Ok(key.to_owned())
    } else {
        quoted(key)
    }
}

fn is_bare_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphanumeric() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn plain_safe(value: &str) -> bool {
    if value.is_empty()
        || value.starts_with(' ')
        || value.ends_with(' ')
        || value.contains('\u{feff}')
    {
        return false;
    }
    if value.chars().any(char::is_control)
        || value.contains('\u{fffe}')
        || value.contains('\u{ffff}')
    {
        return false;
    }
    if value == "true" || value == "false" || value == "null" {
        return false;
    }
    if is_int_syntax(value) || is_float_syntax(value) {
        return false;
    }
    if value.contains(": ") || value.ends_with(':') || value.contains(" #") {
        return false;
    }
    if value.starts_with("---") || value.starts_with("...") {
        return false;
    }

    let first = value.chars().next().expect("value is not empty");
    if matches!(
        first,
        '#' | '"'
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
            | '?'
            | ':'
    ) {
        return false;
    }
    if value == "-" || value.starts_with("- ") {
        return false;
    }

    true
}

fn quoted(value: &str) -> Result<String, Error> {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\t' => output.push_str("\\t"),
            '\u{feff}' => {
                return Err(Error::new(
                    "U+FEFF cannot be represented in a handlist string",
                ));
            }
            '\u{fffe}' | '\u{ffff}' => {
                return Err(Error::new(format!(
                    "noncharacter U+{:04X} cannot be represented in a handlist string",
                    ch as u32
                )));
            }
            ch if ch.is_control() => {
                return Err(Error::new(format!(
                    "control character U+{:04X} cannot be represented in a handlist string",
                    ch as u32
                )));
            }
            _ => output.push(ch),
        }
    }
    output.push('"');
    Ok(output)
}

pub(crate) fn is_block_string(value: &str) -> bool {
    if !value.contains('\n') || value.starts_with('\n') || value.ends_with('\n') {
        return false;
    }
    if value.contains('\r') {
        return false;
    }
    if value
        .chars()
        .any(|ch| ch != '\n' && ch != '\t' && (ch.is_control() || ch == '\u{feff}'))
    {
        return false;
    }
    !value
        .split('\n')
        .any(|line| line.ends_with(' ') || line.ends_with('\t'))
}

pub(crate) fn write_block(value: &str, indent: usize, output: &mut String) {
    for line in value.split('\n') {
        if !line.is_empty() {
            push_indent(output, indent);
            output.push_str(line);
        }
        output.push('\n');
    }
}

fn format_float(value: f64) -> Result<String, Error> {
    if !value.is_finite() {
        return Err(Error::new(
            "non-finite floats cannot be represented in handlist",
        ));
    }

    let mut text = format!("{value}");
    if text.contains('e') || text.contains('E') {
        // Rust's Display implementation does not use exponent notation; this
        // is a defensive fallback for a host-platform difference.
        text = expand_exponent(&text)
            .ok_or_else(|| Error::new(format!("could not format float {text}")))?;
    }
    if !text.contains('.') {
        text.push_str(".0");
    }
    Ok(text)
}

fn expand_exponent(text: &str) -> Option<String> {
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let (mantissa, exponent) = rest.split_once('e').or_else(|| rest.split_once('E'))?;
    let exponent = exponent.parse::<i32>().ok()?;
    let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{integer}{fraction}");
    let point = integer.len() as i32 + exponent;

    let result = if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else if point as usize >= digits.len() {
        format!("{}{}", digits, "0".repeat(point as usize - digits.len()))
    } else {
        format!(
            "{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    };
    Some(format!("{sign}{result}"))
}

pub(crate) fn push_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push(' ');
    }
}
