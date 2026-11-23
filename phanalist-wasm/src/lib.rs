use wasm_bindgen::prelude::*;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub rule: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub severity: String,
}

fn push(findings: &mut Vec<Finding>, rule: &str, message: &str, severity: &str, line: usize) {
    findings.push(Finding {
        rule: rule.to_string(),
        message: message.to_string(),
        line,
        column: 1,
        severity: severity.to_string(),
    });
}

fn analyze_rule(source: &str, rule: &str, findings: &mut Vec<Finding>) {
    let lc = source.to_lowercase();
    for (i, raw_line) in source.lines().enumerate() {
        let line = i + 1;
        let l = raw_line.to_lowercase();
        match rule {
            "E0014" => {
                // Naive: two or more -> in a single expression line (excluding ?->)
                let chain = raw_line.matches("->").count();
                if chain >= 2 && !l.trim_start().starts_with("//") {
                    push(findings, "E0014", "Law of Demeter violation: avoid method chaining on foreign objects.", "warning", line);
                }
            }
            "E0060" => {
                if (l.contains("mysqli_query") || l.contains("->query(") || l.contains("pg_query") || l.contains("pdo::query"))
                    && (l.contains("$_get") || l.contains("$_post") || l.contains("$_request") || l.contains("$id") || l.contains("\" .") || l.contains("' ."))
                {
                    push(findings, "E0060", "Possible SQL injection: user input concatenated into SQL query.", "error", line);
                }
            }
            "E0061" => {
                if (l.contains("echo ") || l.contains("print ")) && (l.contains("$_get") || l.contains("$_post") || l.contains("$_request") || l.contains("$_cookie")) {
                    push(findings, "E0061", "Possible XSS: unescaped user input echoed to browser.", "error", line);
                }
            }
            "E0062" => {
                if (l.contains("system(") || l.contains("exec(") || l.contains("shell_exec(") || l.contains("passthru(") || l.contains("popen("))
                    && (l.contains("$_") || l.contains("\" .") || l.contains("' ."))
                {
                    push(findings, "E0062", "Possible command injection: user input passed to shell execution.", "error", line);
                }
            }
            "E0053" | "E0069" | "E0101" | "E0106" => {
                if l.contains("unserialize(") && !l.contains("allowed_classes") {
                    push(findings, rule, "Unsafe unserialize: untrusted data may cause object injection.", "error", line);
                }
            }
            "E0059" | "E0068" | "E0120" => {
                if l.contains("eval(") {
                    push(findings, rule, "Dynamic eval: dangerous code execution sink.", "error", line);
                }
            }
            _ => {}
        }
    }
    // keep lc used for potential future whole-file rules
    let _ = lc;
}

#[wasm_bindgen]
pub fn analyze(source: &str) -> JsValue {
    analyze_with_rules(source, "E0014")
}

#[wasm_bindgen]
pub fn analyze_with_rules(source: &str, rules: &str) -> JsValue {
    let rules_list: Vec<&str> = rules
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let mut findings = Vec::new();
    for rule in &rules_list {
        analyze_rule(source, rule, &mut findings);
    }
    findings.sort_by(|a, b| a.line.cmp(&b.line).then(a.rule.cmp(&b.rule)));
    let json = serde_json::to_string(&findings).unwrap_or_else(|_| "[]".to_string());
    JsValue::from_str(&json)
}
