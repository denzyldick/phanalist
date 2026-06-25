use mago_span::HasSpan;
use mago_syntax::cst::Statement;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};

pub(crate) static CODE: &str = "E0026";
static DESCRIPTION: &str = "Comment Ratio";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub min_ratio: f64,
    pub max_ratio: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_ratio: 0.1,
            max_ratio: 0.5,
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
        true
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        }
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            let total_lines = file.lines.len();
            if total_lines == 0 {
                return violations;
            }

            let (code_lines, comment_lines) = count_code_and_comment_lines(&file.lines);
            let total_relevant = code_lines + comment_lines;
            if total_relevant == 0 {
                return violations;
            }

            let ratio = comment_lines as f64 / total_relevant as f64;

            if ratio < self.settings.min_ratio {
                let message = Message::new(
                    "E0026:undercommented",
                    "Class \"{name}\" has a comment ratio of {ratio}% (min: {min}%). Add more documentation.",
                )
                .arg("name", String::from_utf8_lossy(class.name.value).to_string())
                .arg("ratio", format!("{:.1}", ratio * 100.0))
                .arg("min", format!("{}", self.settings.min_ratio * 100.0));
                violations.push(self.new_violation(file, message, class.span()));
            } else if ratio > self.settings.max_ratio {
                let message = Message::new(
                    "E0026:overcommented",
                    "Class \"{name}\" has a comment ratio of {ratio}% (max: {max}%). Too many comments may indicate unclear code.",
                )
                .arg("name", String::from_utf8_lossy(class.name.value).to_string())
                .arg("ratio", format!("{:.1}", ratio * 100.0))
                .arg("max", format!("{}", self.settings.max_ratio * 100.0));
                violations.push(self.new_violation(file, message, class.span()));
            }
        }

        violations
    }
}

fn count_code_and_comment_lines(lines: &[String]) -> (usize, usize) {
    let mut code_lines = 0;
    let mut comment_lines = 0;
    let mut in_block_comment = false;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if in_block_comment {
            comment_lines += 1;
            if let Some(pos) = trimmed.find("*/") {
                let after_close = trimmed[pos + 2..].trim();
                if !after_close.is_empty() {
                    code_lines += 1;
                }
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            comment_lines += 1;
        } else if let Some(rest) = trimmed.strip_prefix("/*") {
            comment_lines += 1;
            if let Some(close_pos) = rest.find("*/") {
                let after_close = rest[close_pos + 2..].trim();
                if !after_close.is_empty() {
                    code_lines += 1;
                }
            } else {
                in_block_comment = true;
            }
        } else {
            code_lines += 1;
        }
    }

    if in_block_comment {
        comment_lines += 1;
    }

    (code_lines, comment_lines)
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn undercommented() {
        let violations = analyze_file_for_rule("e26/undercommented.php", CODE);
        assert!(violations.len().gt(&0));
        assert!(violations[0].message.render().contains("10%"));
    }

    #[test]
    fn overcommented() {
        let violations = analyze_file_for_rule("e26/overcommented.php", CODE);
        assert!(violations.len().gt(&0));
        assert!(violations[0].message.render().contains("50%"));
    }

    #[test]
    fn well_commented() {
        let violations = analyze_file_for_rule("e26/well_commented.php", CODE);
        assert!(violations.len().eq(&0));
    }
}
