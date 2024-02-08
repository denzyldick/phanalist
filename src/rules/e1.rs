use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0001";
static DESCRIPTION: &str = "Opening tag position";

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

        match statement {
            Statement::FullOpeningTag(tag) => {
                let span = tag.span;
                if span.line > 1 {
                    let suggestion= String::from("The opening tag is not on the right line. This should always be the first line in a PHP file.");
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
                    let suggestion= String::from("The opening tag is not on the right line. This should always be the first line in a PHP file.");
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

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn full_opening_tag_valid() {
        let violations = analyze_file_for_rule("e1/full_opening_tag_valid.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn full_opening_tag_not_first_line() {
        let violations = analyze_file_for_rule("e1/full_opening_tag_not_first_line.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(violations.first().unwrap().suggestion, "The opening tag is not on the right line. This should always be the first line in a PHP file.".to_string());
    }

    #[test]
    fn test_full_opening_tag_not_first_column() {
        let violations = analyze_file_for_rule("e1/full_opening_tag_not_first_column.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "The opening tag doesn't start at the right column: 2.".to_string()
        );
    }

    #[test]
    fn short_opening_tag_valid() {
        let violations = analyze_file_for_rule("e1/short_opening_tag_valid.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn short_opening_tag_not_first_line() {
        let violations = analyze_file_for_rule("e1/short_opening_tag_not_first_line.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(violations.first().unwrap().suggestion, "The opening tag is not on the right line. This should always be the first line in a PHP file.".to_string());
    }

    #[test]
    fn short_full_opening_tag_not_first_column() {
        let violations = analyze_file_for_rule("e1/short_opening_tag_not_first_column.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "The opening tag doesn't start at the right column: 2.".to_string()
        );
    }
}
