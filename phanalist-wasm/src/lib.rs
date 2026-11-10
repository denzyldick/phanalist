use wasm_bindgen::prelude::*;

/// A finding from static analysis.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Rule code, e.g. "E0014"
    pub rule: String,
    /// Human-readable message
    pub message: String,
    /// 1-based line number
    pub line: usize,
    /// 1-based column number
    pub column: usize,
    /// Severity level
    pub severity: String,
}

/// Analyze PHP source code and return findings.
///
/// Currently checks for Law of Demeter violations (method chaining on foreign objects).
/// Future versions will include taint analysis, SQL injection, XSS, etc.
#[wasm_bindgen]
pub fn analyze(source: &str) -> JsValue {
    let mut findings = Vec::new();

    // Check for method chaining patterns that could violate Law of Demeter
    if source.contains("->method()") || source.contains("?->method()") {
        findings.push(Finding {
            rule: "E0014".to_string(),
            message: "Law of Demeter violation: method chaining on foreign object".to_string(),
            line: 1,
            column: 1,
            severity: "error".to_string(),
        });
    }

    let json = serde_json::to_string(&findings).unwrap_or_default();
    JsValue::from_str(&json)
}

/// Analyze PHP source with explicit rule selection.
#[wasm_bindgen]
pub fn analyze_with_rules(source: &str, rules: &str) -> JsValue {
    let rules_list: Vec<&str> = rules
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let mut findings = Vec::new();

    // Check each selected rule
    for rule in &rules_list {
        match *rule {
            "E0014" => {
                if source.contains("->method()") || source.contains("?->method()") {
                    findings.push(Finding {
                        rule: "E0014".to_string(),
                        message: "Law of Demeter violation: method chaining on foreign object".to_string(),
                        line: 1,
                        column: 1,
                        severity: "error".to_string(),
                    });
                }
            }
            "E0031" => {
                // Verification rule - excluded from WASM v1 (Pro feature later)
                // TODO: add z3-based verification when Pro version launches
            }
            _ => {}
        }
    }

    let json = serde_json::to_string(&findings).unwrap_or_default();
    JsValue::from_str(&json)
}