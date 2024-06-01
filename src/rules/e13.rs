use php_parser_rs::parser::ast::{Statement};


use crate::file::{File};
use crate::results::Violation;

static CODE: &str = "E0013";
static DESCRIPTION: &str = "Private method not being called.";

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();
        if let Statement::Class(_e) = statement {
            for (_key, value) in &file.reference_counter.methods {
                let zero: isize = 0;
                let message = format!(
                    "The private method {} is not being called. ",
                    value.name
                );
                if &value.counter == &zero {
                    violations.push(self.new_violation(file, message, value.span));
                }
            }
        }
        violations
    }
    fn do_validate(&self, file: &File) -> bool {
        file.get_fully_qualified_name().is_some()
    }

    fn new_violation(
        &self,
        file: &File,
        suggestion: String,
        span: php_parser_rs::lexer::token::Span,
    ) -> Violation {
        let line = file.lines.get(span.line - 1).unwrap();

        Violation {
            rule: self.get_code(),
            line: String::from(line),
            suggestion,
            span,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e13/private_method_not_being_called.php", CODE);

        println!("{}", violations.len());
        assert!(violations.len().eq(&1));
    }
}
