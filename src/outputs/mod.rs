use crate::results::Results;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

pub mod codeclimate;
pub mod text;
pub mod json;
pub mod sarif;

pub trait OutputFormatter {
    fn output(_results: &mut Results) {}
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
