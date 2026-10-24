use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
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
