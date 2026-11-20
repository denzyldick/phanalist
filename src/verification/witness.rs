//! Concrete counterexamples extracted from a satisfiable Z3 model.
//!
//! A verification finding normally only carries a rendered message with one or
//! two substituted values. When test generation is enabled we also capture the
//! parameter values that make the failing path reachable, so a regression test
//! can call the real function with the same inputs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use z3::Model;

use super::value::{ArrayKey, Value};

/// Maximum length we are willing to render as a PHP literal. A solver model is
/// free to pick an enormous length; generating `str_repeat('a', 10^9)` would be
/// worse than useless.
const MAX_RENDERED_LENGTH: i64 = 64;

/// A parameter value that the model can pin down well enough to put in a test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConcreteValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    /// A string of exactly this many bytes, rendered with `str_repeat`.
    StringOfLength(u32),
    /// A known literal string.
    StringLiteral(String),
    /// A list/array of exactly this many elements, rendered with `array_fill`.
    ArrayOfLength(u32),
    /// An array whose tracked keys are known; values are filled with `null`.
    ArrayKeys(Vec<ArrayKey>),
    Null,
    /// The model cannot give us a value that can be written as a PHP literal.
    Unsupported(String),
}

impl ConcreteValue {
    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported(_))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WitnessInput {
    /// PHP parameter name, including the leading `$`.
    pub name: String,
    pub value: ConcreteValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallableKind {
    Function,
    StaticMethod,
    InstanceMethod,
}

/// Constructor inputs needed to build the receiver of an instance-method test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WitnessReceiver {
    /// Fully qualified class name, without a leading backslash.
    pub class: String,
    pub arguments: Vec<WitnessInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallableContext {
    /// Fully qualified name, without a leading backslash.
    pub name: String,
    pub kind: CallableKind,
    pub file: PathBuf,
    pub line: usize,
}

/// Everything needed to render a runnable test for one finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Witness {
    pub code: String,
    pub message: String,
    pub source: PathBuf,
    pub line: usize,
    pub callable: Option<CallableContext>,
    pub inputs: Vec<WitnessInput>,
    /// Present only for instance methods whose class can be constructed from
    /// modeled inputs.
    pub receiver: Option<WitnessReceiver>,
    /// Inferred route/caller chain that reaches this callable. Only used in
    /// the generated test docblock; never serialized in reports.
    #[serde(default)]
    pub call_chain: Vec<String>,
}

impl Witness {
    /// True when the model did not pin down every input well enough to build a
    /// trustworthy test.
    pub fn has_unsupported_input(&self) -> bool {
        self.callable.is_none()
            || self.callable.as_ref().is_some_and(|callable| {
                callable.kind == CallableKind::InstanceMethod && self.receiver.is_none()
            })
            || self.inputs.iter().any(|input| input.value.is_unsupported())
            || self.receiver.as_ref().is_some_and(|receiver| {
                receiver
                    .arguments
                    .iter()
                    .any(|input| input.value.is_unsupported())
            })
    }
}

/// Evaluate a symbolic value under a satisfying model.
pub(crate) fn concrete_value(model: &Model, value: &Value) -> ConcreteValue {
    match value {
        Value::Int(symbolic) => model
            .eval(symbolic, true)
            .and_then(|value| value.as_i64())
            .map(ConcreteValue::Int)
            .unwrap_or_else(|| ConcreteValue::Unsupported("integer value is not concrete".into())),
        Value::Float(symbolic) => model
            .eval(symbolic, true)
            .map(|value| value.as_f64())
            .filter(|value| value.is_finite())
            .map(ConcreteValue::Float)
            .unwrap_or_else(|| ConcreteValue::Unsupported("float value is not concrete".into())),
        Value::Bool(symbolic) => model
            .eval(symbolic, true)
            .and_then(|value| value.as_bool())
            .map(ConcreteValue::Bool)
            .unwrap_or_else(|| ConcreteValue::Unsupported("boolean value is not concrete".into())),
        Value::StringLength(symbolic) => bounded_length(model, symbolic)
            .map(ConcreteValue::StringOfLength)
            .unwrap_or_else(|| ConcreteValue::Unsupported("string length is not concrete".into())),
        Value::ListLength(symbolic) | Value::ArrayLength(symbolic) => {
            bounded_length(model, symbolic)
                .map(ConcreteValue::ArrayOfLength)
                .unwrap_or_else(|| {
                    ConcreteValue::Unsupported("array length is not concrete".into())
                })
        }
        Value::UnverifiedNumeric {
            loop_accumulator: true,
            ..
        } => ConcreteValue::Unsupported("value depends on an unrolled loop accumulator".into()),
        Value::UnverifiedNumeric {
            value: symbolic, ..
        } => model
            .eval(symbolic, true)
            .and_then(|value| value.as_i64())
            .map(ConcreteValue::Int)
            .unwrap_or_else(|| ConcreteValue::Unsupported("numeric value is not concrete".into())),
        // An unknown string/array/object would otherwise block the whole test.
        // An empty string or array is a faithful placeholder for the findings
        // we generate: out-of-bounds and missing-key paths stay reachable.
        Value::String(None) => ConcreteValue::StringOfLength(0),
        Value::String(Some(value)) => ConcreteValue::StringLiteral(value.clone()),
        Value::Array(None) => ConcreteValue::ArrayOfLength(0),
        Value::Array(Some(keys)) => ConcreteValue::ArrayKeys(keys.iter().cloned().collect()),
        Value::Null => ConcreteValue::Null,
        Value::Object(_) => ConcreteValue::Unsupported("objects are not modeled".into()),
        Value::Resource { .. } => ConcreteValue::Unsupported("resources are not modeled".into()),
        Value::Unknown | Value::Unverified => {
            ConcreteValue::Unsupported("value is not modeled".into())
        }
    }
}

fn bounded_length(model: &Model, symbolic: &z3::ast::Int) -> Option<u32> {
    let length = model
        .eval(symbolic, true)
        .and_then(|value| value.as_i64())?;
    (length >= 0 && length <= MAX_RENDERED_LENGTH).then_some(length as u32)
}

#[cfg(test)]
mod tests {
    use z3::{
        ast::{Bool, Int},
        Solver,
    };

    use super::*;

    #[test]
    fn extracts_int_and_bool_values_from_a_model() {
        let zero = Int::from_i64(0);
        let flag = Bool::from_bool(true);
        let solver = Solver::new();
        solver.assert(&zero.eq(0));
        solver.assert(&flag);
        assert_eq!(solver.check(), z3::SatResult::Sat);
        let model = solver.get_model().unwrap();

        assert_eq!(
            concrete_value(&model, &Value::Int(Int::from_i64(7))),
            ConcreteValue::Int(7)
        );
        assert_eq!(
            concrete_value(&model, &Value::Bool(Bool::from_bool(true))),
            ConcreteValue::Bool(true)
        );
    }

    #[test]
    fn unsupported_values_are_explicit() {
        let solver = Solver::new();
        solver.assert(&Bool::from_bool(true));
        assert_eq!(solver.check(), z3::SatResult::Sat);
        let model = solver.get_model().unwrap();

        assert!(concrete_value(
            &model,
            &Value::UnverifiedNumeric {
                value: Int::from_i64(1),
                loop_accumulator: true,
            }
        )
        .is_unsupported());
        assert!(concrete_value(&model, &Value::Object("Demo\\Thing".into())).is_unsupported());
    }

    #[test]
    fn containers_get_renderable_placeholders() {
        let solver = Solver::new();
        solver.assert(&Bool::from_bool(true));
        assert_eq!(solver.check(), z3::SatResult::Sat);
        let model = solver.get_model().unwrap();

        assert_eq!(
            concrete_value(&model, &Value::String(None)),
            ConcreteValue::StringOfLength(0)
        );
        assert_eq!(
            concrete_value(&model, &Value::String(Some("hello".into()))),
            ConcreteValue::StringLiteral("hello".into())
        );
        assert_eq!(
            concrete_value(&model, &Value::Array(None)),
            ConcreteValue::ArrayOfLength(0)
        );
        assert_eq!(
            concrete_value(
                &model,
                &Value::Array(Some(
                    [ArrayKey::Int(1), ArrayKey::String("key".into())].into()
                ))
            ),
            ConcreteValue::ArrayKeys(vec![ArrayKey::Int(1), ArrayKey::String("key".into())])
        );
        assert_eq!(concrete_value(&model, &Value::Null), ConcreteValue::Null);
    }

    #[test]
    fn instance_witnesses_need_a_renderable_receiver() {
        let make = |receiver| Witness {
            code: "E0031".to_string(),
            message: "Division by zero".to_string(),
            source: PathBuf::from("src/Calc.php"),
            line: 3,
            callable: Some(CallableContext {
                name: "Calc::divide".to_string(),
                kind: CallableKind::InstanceMethod,
                file: PathBuf::from("src/Calc.php"),
                line: 2,
            }),
            inputs: vec![WitnessInput {
                name: "$d".to_string(),
                value: ConcreteValue::Int(0),
            }],
            receiver,
            call_chain: Vec::new(),
        };

        assert!(make(None).has_unsupported_input());
        assert!(!make(Some(WitnessReceiver {
            class: "Calc".to_string(),
            arguments: Vec::new(),
        }))
        .has_unsupported_input());
        assert!(make(Some(WitnessReceiver {
            class: "Calc".to_string(),
            arguments: vec![WitnessInput {
                name: "$label".to_string(),
                value: ConcreteValue::Unsupported("object".to_string()),
            }],
        }))
        .has_unsupported_input());
    }
}
