use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0001")
    }

    fn description(&self) -> String {
        String::from("Opening tag position")
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        match statement {
            Statement::FullOpeningTag(tag) => {
                let span = tag.span;
                if span.line > 1 {
                    let suggestion= String::from("The opening tag <?php is not on the right line. This should always be the first line in a PHP file.");
                    violations.push(self.new_violation(file, suggestion, span));
                }

                if span.column > 1 {
                    let suggestion = format!(
                        "The opening tag doesn't start at the right column: {}.",
                        span.column
                    );
                    violations.push(self.new_violation(file, suggestion, span));
                }
            }
            Statement::ShortOpeningTag(tag) => {
                let span = tag.span;
                if span.line > 1 {
                    let suggestion= String::from("The opening tag <?php is not on the right line. This should always be the first line in a PHP file.");
                    violations.push(self.new_violation(file, suggestion, span));
                }

                if span.column > 1 {
                    let suggestion = format!(
                        "The opening tag doesn't start at the right column: {}.",
                        span.column
                    );
                    violations.push(self.new_violation(file, suggestion, span));
                }
            }

            _ => {}
        };

        violations
    }
}
