use std::collections::BTreeSet;

use z3::ast::{Bool, Float, Int};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArrayKey {
    Int(i64),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(Int),
    Float(Float),
    Bool(Bool),
    String(Option<String>),
    StringLength(Int),
    Array(Option<BTreeSet<ArrayKey>>),
    ArrayLength(Int),
    ListLength(Int),
    Null,
    Unknown,
    Unverified,
    UnverifiedNumeric { value: Int, loop_accumulator: bool },
    Object(String),
}

impl Value {
    pub fn zero_condition(&self) -> Option<Bool> {
        match self {
            Value::Int(value) => Some(value.eq(0)),
            Value::Float(value) => Some(value.is_zero()),
            Value::UnverifiedNumeric { value, .. } => Some(value.eq(0)),
            _ => None,
        }
    }

    pub fn truthiness(&self) -> Option<Bool> {
        match self {
            Value::Bool(value) => Some(value.clone()),
            Value::Int(value) => Some(value.eq(0).not()),
            Value::Float(value) => Some(value.is_zero().not()),
            Value::Null => Some(Bool::from_bool(false)),
            Value::String(Some(value)) => Some(Bool::from_bool(!value.is_empty() && value != "0")),
            Value::StringLength(_) => None,
            Value::Array(Some(keys)) => Some(Bool::from_bool(!keys.is_empty())),
            Value::ArrayLength(length) | Value::ListLength(length) => Some(length.eq(0).not()),
            Value::UnverifiedNumeric { value, .. } => Some(value.eq(0).not()),
            Value::Object(_) => Some(Bool::from_bool(true)),
            Value::String(None) | Value::Array(None) | Value::Unknown | Value::Unverified => None,
        }
    }

    pub fn to_float(&self) -> Option<Float> {
        match self {
            Value::Float(value) => Some(value.clone()),
            Value::Int(value) => {
                let literal = value.as_i64()?;
                if literal.unsigned_abs() > (1_u64 << 53) {
                    return None;
                }
                Some(Float::from_f64(literal as f64))
            }
            _ => None,
        }
    }

    pub fn merge(condition: Option<&Bool>, left: &Value, right: &Value) -> Value {
        if condition.is_none() && left == right {
            return left.clone();
        }
        match (condition, left, right) {
            (Some(condition), Value::Int(left), Value::Int(right)) => {
                Value::Int(condition.ite(left, right))
            }
            (Some(condition), Value::Float(left), Value::Float(right)) => {
                Value::Float(condition.ite(left, right))
            }
            (Some(condition), Value::Bool(left), Value::Bool(right)) => {
                Value::Bool(condition.ite(left, right))
            }
            (
                Some(condition),
                Value::UnverifiedNumeric {
                    value: left,
                    loop_accumulator: left_loop,
                },
                Value::UnverifiedNumeric {
                    value: right,
                    loop_accumulator: right_loop,
                },
            ) => Value::UnverifiedNumeric {
                value: condition.ite(left, right),
                loop_accumulator: *left_loop || *right_loop,
            },
            (
                Some(condition),
                Value::UnverifiedNumeric {
                    value: left,
                    loop_accumulator,
                },
                Value::Int(right),
            ) => Value::UnverifiedNumeric {
                value: condition.ite(left, right),
                loop_accumulator: *loop_accumulator,
            },
            (
                Some(condition),
                Value::Int(left),
                Value::UnverifiedNumeric {
                    value: right,
                    loop_accumulator,
                },
            ) => Value::UnverifiedNumeric {
                value: condition.ite(left, right),
                loop_accumulator: *loop_accumulator,
            },
            (Some(_), Value::Object(left), Value::Object(right)) if left == right => {
                Value::Object(left.clone())
            }
            (None, Value::Object(left), Value::Object(right)) if left == right => {
                Value::Object(left.clone())
            }
            (None, Value::String(Some(left)), Value::String(Some(right))) if left == right => {
                Value::String(Some(left.clone()))
            }
            (None, Value::Null, Value::Null) => Value::Null,
            (Some(condition), Value::StringLength(left), Value::StringLength(right)) => {
                Value::StringLength(condition.ite(left, right))
            }
            (Some(condition), Value::ArrayLength(left), Value::ArrayLength(right)) => {
                Value::ArrayLength(condition.ite(left, right))
            }
            (Some(condition), Value::ListLength(left), Value::ListLength(right)) => {
                Value::ListLength(condition.ite(left, right))
            }
            (Some(condition), Value::Array(Some(left)), Value::Array(Some(right)))
                if left == right =>
            {
                let _ = condition;
                Value::Array(Some(left.clone()))
            }
            (None, Value::Array(Some(left)), Value::Array(Some(right))) if left == right => {
                Value::Array(Some(left.clone()))
            }
            _ => Value::Unknown,
        }
    }
}
