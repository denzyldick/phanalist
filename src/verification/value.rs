use z3::ast::{Bool, Float, Int};

#[derive(Clone, Debug)]
pub enum Value {
    Int(Int),
    Float(Float),
    Bool(Bool),
    String(Option<String>),
    Null,
    Unknown,
}

impl Value {
    pub fn zero_condition(&self) -> Option<Bool> {
        match self {
            Value::Int(value) => Some(value.eq(0)),
            Value::Float(value) => Some(value.is_zero()),
            _ => None,
        }
    }

    pub fn truthiness(&self) -> Option<Bool> {
        match self {
            Value::Bool(value) => Some(value.clone()),
            Value::Int(value) => Some(value.eq(0).not()),
            Value::Float(value) => Some(value.is_zero().not()),
            Value::Null => Some(Bool::from_bool(false)),
            Value::String(Some(value)) => {
                Some(Bool::from_bool(!value.is_empty() && value != "0"))
            }
            Value::String(None) | Value::Unknown => None,
        }
    }

    pub fn as_literal_i64(&self) -> Option<i64> {
        match self {
            Value::Int(value) => value.as_i64(),
            _ => None,
        }
    }

    pub fn as_literal_f64(&self) -> Option<f64> {
        match self {
            Value::Float(value) => Some(value.as_f64()),
            Value::Int(value) => value.as_i64().map(|value| value as f64),
            _ => None,
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
            (None, Value::String(Some(left)), Value::String(Some(right))) if left == right => {
                Value::String(Some(left.clone()))
            }
            (None, Value::Null, Value::Null) => Value::Null,
            _ => Value::Unknown,
        }
    }
}
