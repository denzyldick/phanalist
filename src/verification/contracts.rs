use std::collections::HashMap;

use z3::ast::{Ast, Bool, Int};

use super::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Assertion {
    pub parameter: String,
    pub ty: String,
    pub negated: bool,
}

#[derive(Debug, Default, Clone)]
pub struct Docblock {
    pub params: HashMap<String, String>,
    pub vars: HashMap<String, String>,
    pub var: Option<String>,
    pub asserts: Vec<Assertion>,
    pub return_type: Option<String>,
}

impl Docblock {
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
            && self.vars.is_empty()
            && self.var.is_none()
            && self.asserts.is_empty()
            && self.return_type.is_none()
    }
}

pub fn parse_docblock(raw: &[u8]) -> Docblock {
    let text = String::from_utf8_lossy(raw);
    let mut docblock = Docblock::default();

    for line in text.lines() {
        let line = line
            .trim()
            .trim_start_matches('/')
            .trim_start_matches('*')
            .trim();
        if let Some(return_type) =
            tagged_type(line, "@phpstan-return").or_else(|| tagged_type(line, "@psalm-return"))
        {
            docblock.return_type = Some(return_type);
        } else if let Some(return_type) = tagged_type(line, "@return") {
            docblock.return_type.get_or_insert(return_type);
        } else if let Some((name, ty)) = tagged_param(line, "@phpstan-param")
            .or_else(|| tagged_param(line, "@psalm-param"))
            .or_else(|| tagged_param(line, "@param"))
        {
            docblock.params.entry(name).or_insert(ty);
        } else if let Some((name, ty)) = tagged_var(line, "@phpstan-var")
            .or_else(|| tagged_var(line, "@psalm-var"))
            .or_else(|| tagged_var(line, "@var"))
        {
            if let Some(name) = name {
                docblock.vars.entry(name).or_insert(ty);
            } else {
                docblock.var.get_or_insert(ty);
            }
        } else if let Some(assertion) = tagged_assert(line, "@phpstan-assert")
            .or_else(|| tagged_assert(line, "@psalm-assert"))
            .or_else(|| tagged_assert(line, "@assert"))
        {
            docblock.asserts.push(assertion);
        }
    }

    docblock
}

fn tagged_type(line: &str, tag: &str) -> Option<String> {
    let rest = line.strip_prefix(tag)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let ty = rest.trim().split_whitespace().next()?.trim();
    if ty.is_empty() {
        None
    } else {
        Some(normalize(ty))
    }
}

fn tagged_param(line: &str, tag: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix(tag)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let dollar = rest.find('$')?;
    let mut ty = rest[..dollar].trim().trim_end_matches('&').trim();
    ty = ty.trim_end_matches("...").trim();
    if ty.is_empty() {
        return None;
    }
    let name = rest[dollar + 1..]
        .split(|c: char| c.is_whitespace() || c == ',')
        .next()?;
    if name.is_empty() {
        return None;
    }
    Some((name.to_string(), normalize(ty)))
}

fn tagged_assert(line: &str, tag: &str) -> Option<Assertion> {
    let rest = line.strip_prefix(tag)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let dollar = rest.find('$')?;
    let mut ty = rest[..dollar].trim();
    if ty.is_empty() {
        return None;
    }
    let negated = ty.starts_with('!');
    ty = ty.trim_start_matches('!').trim();
    if ty.is_empty() {
        return None;
    }
    let parameter = rest[dollar + 1..]
        .split(|c: char| c.is_whitespace() || c == ',')
        .next()?;
    if parameter.is_empty() {
        return None;
    }
    Some(Assertion {
        parameter: format!("${parameter}"),
        ty: normalize(ty),
        negated,
    })
}

fn tagged_var(line: &str, tag: &str) -> Option<(Option<String>, String)> {
    let rest = line.strip_prefix(tag)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }

    if let Some(dollar) = rest.find('$') {
        let ty = rest[..dollar].trim();
        if ty.is_empty() {
            return None;
        }
        let name = rest[dollar + 1..]
            .split(|c: char| c.is_whitespace() || c == ',')
            .next()?;
        if name.is_empty() {
            return None;
        }
        return Some((Some(format!("${name}")), normalize(ty)));
    }

    let ty = rest.split_whitespace().next()?.trim();
    if ty.is_empty() {
        None
    } else {
        Some((None, normalize(ty)))
    }
}

fn normalize(ty: &str) -> String {
    ty.chars().filter(|c| !c.is_whitespace()).collect()
}

fn split_union(ty: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    for (index, byte) in ty.bytes().enumerate() {
        match byte {
            b'<' | b'(' | b'{' | b'[' => depth += 1,
            b'>' | b')' | b'}' | b']' => depth -= 1,
            b'|' if depth == 0 => {
                parts.push(&ty[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&ty[start..]);
    parts
}

pub fn is_nullable(ty: &str) -> bool {
    let ty = normalize(ty);
    if ty.starts_with('?') || ty == "mixed" {
        return true;
    }
    split_union(&ty)
        .iter()
        .any(|part| *part == "null" || part.starts_with('?'))
}

pub fn accepts_value(value: &Value, ty: &str, strict: bool) -> Option<bool> {
    if matches!(value, Value::Unknown) {
        return None;
    }

    let ty = normalize(ty);
    if ty.is_empty() {
        return None;
    }

    if let Some(inner) = ty.strip_prefix('?') {
        if matches!(value, Value::Null) {
            return Some(true);
        }
        return accepts_value(value, inner, strict);
    }

    let members = split_union(&ty);
    if members.len() > 1 {
        let mut unknown = false;
        for member in members {
            match accepts_value(value, member, strict) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => unknown = true,
            }
        }
        return if unknown { None } else { Some(false) };
    }

    if let Some(inner) = ty.strip_prefix('(').and_then(|ty| ty.strip_suffix(')')) {
        return accepts_value(value, inner, strict);
    }

    match ty.as_str() {
        "mixed" => Some(true),
        "null" | "void" | "never" => Some(matches!(value, Value::Null)),
        "bool" | "boolean" => Some(matches!(value, Value::Bool(_))),
        "true" => Some(matches!(value, Value::Bool(value) if value.as_bool() == Some(true))),
        "false" => Some(matches!(value, Value::Bool(value) if value.as_bool() == Some(false))),
        "int" | "integer" => Some(matches!(value, Value::Int(_))),
        "float" | "double" => Some(match value {
            Value::Float(_) => true,
            Value::Int(_) => !strict,
            _ => false,
        }),
        "numeric" => match value {
            Value::Int(_) | Value::Float(_) => Some(true),
            Value::String(Some(value)) => Some(value.parse::<f64>().is_ok()),
            Value::String(None) => None,
            _ => Some(false),
        },
        "scalar" => Some(matches!(
            value,
            Value::Int(_) | Value::Float(_) | Value::Bool(_) | Value::String(_)
        )),
        "string" => Some(matches!(value, Value::String(_))),
        "non-empty-string" => match value {
            Value::String(Some(value)) => Some(!value.is_empty()),
            Value::String(None) => None,
            _ => Some(false),
        },
        "numeric-string" => match value {
            Value::String(Some(value)) => Some(value.parse::<f64>().is_ok()),
            Value::String(None) => None,
            _ => Some(false),
        },
        "array-key" => Some(matches!(value, Value::Int(_) | Value::String(_))),
        "array" | "iterable" => Some(matches!(value, Value::Array(_))),
        "non-empty-array" | "non-empty-list" => match value {
            Value::Array(Some(keys)) => Some(!keys.is_empty()),
            Value::Array(None) => None,
            _ => Some(false),
        },
        "positive-int" | "non-negative-int" | "negative-int" | "non-positive-int"
        | "non-zero-int" => literal_int(value).map(|value| match ty.as_str() {
            "positive-int" => value > 0,
            "non-negative-int" => value >= 0,
            "negative-int" => value < 0,
            "non-positive-int" => value <= 0,
            "non-zero-int" => value != 0,
            _ => false,
        }),
        _ if ty.starts_with("int<") => int_range(&ty)
            .and_then(|(min, max)| literal_int(value).map(|value| value >= min && value <= max)),
        _ if ty.starts_with("list<") || ty.starts_with("array<") => {
            Some(matches!(value, Value::Array(_)))
        }
        _ => None,
    }
}

fn literal_int(value: &Value) -> Option<i64> {
    match value {
        Value::Int(value) => value.as_i64().or_else(|| value.simplify().as_i64()),
        _ => None,
    }
}

fn int_range(ty: &str) -> Option<(i64, i64)> {
    let inner = ty.strip_prefix("int<")?.strip_suffix('>')?;
    let (min, max) = inner.split_once(',')?;
    let min = parse_bound(min)?;
    let max = parse_bound(max)?;
    Some((min, max))
}

fn parse_bound(value: &str) -> Option<i64> {
    if value == "min" {
        Some(i64::MIN)
    } else if value == "max" {
        Some(i64::MAX)
    } else {
        value.parse().ok()
    }
}

pub fn constraint(value: &Value, ty: &str) -> Option<Bool> {
    let ty = normalize(ty);
    if ty.starts_with('?') {
        return None;
    }

    let members = split_union(&ty);
    if members.len() > 1 {
        let constraints = members
            .iter()
            .filter_map(|member| constraint(value, member))
            .collect::<Vec<_>>();
        if constraints.is_empty() {
            return None;
        }
        return Some(Bool::or(&constraints.iter().collect::<Vec<_>>()));
    }

    let Value::Int(integer) = value else {
        return match ty.as_str() {
            "non-empty-string" => match value {
                Value::String(Some(value)) => Some(Bool::from_bool(!value.is_empty())),
                _ => None,
            },
            "non-empty-array" | "non-empty-list" => match value {
                Value::Array(Some(keys)) => Some(Bool::from_bool(!keys.is_empty())),
                _ => None,
            },
            _ => None,
        };
    };

    match ty.as_str() {
        "positive-int" => Some(integer.gt(Int::from_i64(0))),
        "non-negative-int" => Some(integer.ge(Int::from_i64(0))),
        "negative-int" => Some(integer.lt(Int::from_i64(0))),
        "non-positive-int" => Some(integer.le(Int::from_i64(0))),
        "non-zero-int" => Some(integer.eq(Int::from_i64(0)).not()),
        _ if ty.starts_with("int<") => {
            let (min, max) = int_range(&ty)?;
            Some(Bool::and(&[
                &integer.ge(Int::from_i64(min)),
                &integer.le(Int::from_i64(max)),
            ]))
        }
        _ => None,
    }
}
