use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::walk_statement_expressions;

pub(crate) static CODE: &str = "E0011";
static DESCRIPTION: &str = "Detect the error suppression symbol: @";

#[derive(Default)]
pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, file: &File<'_>) -> bool {
        file.ast.is_some()
    }

    fn validate(&self, _file: &File<'_>, _statement: &Statement<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn validate_file(&self, file: &File<'_>) -> Option<Vec<Violation>> {
        let program = file.ast?;
        let mut violations = Vec::new();

        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| {
                if let Expression::UnaryPrefix(prefix) = expression {
                    if matches!(prefix.operator, UnaryPrefixOperator::ErrorControl(_)) {
                        let message = Message::new(
                            "E0011:error-suppression",
                            "Error supression(@) symbol found. Remove it.",
                        );
                        violations.push(self.new_violation(file, message, prefix.span()));
                    }
                }
            });
        }

        Some(violations)
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::{analyze_file_for_rule, analyze_source_for_rule};

    use super::*;

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e11/detect_@.php", CODE);

        assert!(violations.len().gt(&0));
    }

    #[test]
    fn detects_error_suppression_in_nested_expressions() {
        let source = r#"<?php
final class Example {
    public function run(): void {
        $content = @file_get_contents('file.txt');
        if (@$content) {
            echo @trim($content);
        }
    }
}
"#;

        let violations = analyze_source_for_rule(source, "e11-inline.php", CODE);
        assert_eq!(violations.len(), 3);
    }
}
