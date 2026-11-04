use crate::results::Results;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

pub mod chart;
pub mod codeclimate;
pub mod json;
pub mod sarif;
pub mod text;

pub trait OutputFormatter {
    fn output(_results: &mut Results) {}

    fn output_with_settings(
        _results: &mut Results,
        _settings: &HashMap<String, serde_json::Value>,
    ) {
    }

    fn output_with_effective(
        _results: &mut Results,
        _settings: &HashMap<String, serde_json::Value>,
    ) {
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_camel_case_types)]
pub enum Format {
    text,
    json,
    sarif,
    codeclimate,
}

impl FromStr for Format {
    type Err = ();

    fn from_str(input: &str) -> Result<Format, Self::Err> {
        match input {
            "text" => Ok(Format::text),
            "json" => Ok(Format::json),
            "sarif" => Ok(Format::sarif),
            "codeclimate" => Ok(Format::codeclimate),
            _ => Err(()),
        }
    }
}
