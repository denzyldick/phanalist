use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;

pub(crate) static CODE: &str = "E0031";
static DESCRIPTION: &str = "Formal Verification";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
#[allow(dead_code)]
pub struct Settings {
    pub php_version: String,
    pub check_division: bool,
    pub check_modulo: bool,
    pub check_intdiv: bool,
    pub check_array_index: bool,
    pub check_string_offset: bool,
    pub check_nullability: bool,
    pub check_types: bool,
    pub check_assertions: bool,
    pub check_unreachable: bool,
    pub check_return_contracts: bool,
    pub loop_strategy: String,
    pub unroll_depth: usize,
    pub timeout_ms: u64,
    pub unknown_policy: String,
    pub max_findings_per_function: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            php_version: String::from("8.4"),
            check_division: true,
            check_modulo: true,
            check_intdiv: true,
            check_array_index: true,
            check_string_offset: true,
            check_nullability: true,
            check_types: true,
            check_assertions: true,
            check_unreachable: true,
            check_return_contracts: true,
            loop_strategy: String::from("auto"),
            unroll_depth: 16,
            timeout_ms: 1_000,
            unknown_policy: String::from("report"),
            max_findings_per_function: 50,
        }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        false
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        }
    }

    fn validate(&self, _file: &File<'_>, _statement: &mago_syntax::cst::Statement<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn validate_file(&self, _file: &File<'_>) -> Option<Vec<Violation>> {
        Some(Vec::new())
    }
}
