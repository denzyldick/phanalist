use mago_syntax::ast::{OpeningTag, Statement};

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

    fn do_validate(&self, _file: &File<'_>) -> bool {
        true
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Some(line) = file.lines.first() {
            if line.trim().starts_with("#!") {
                return violations;
            }
        }

        if let Statement::OpeningTag(tag) = statement {
            let span = match tag {
                OpeningTag::Full(t) => t.span,
                OpeningTag::Short(t) => t.span,
            };

            let line = file.line_number(span.start.offset);
            let column = file.column_number(span.start.offset);

            if line > 1 {
                let suggestion = String::from(
                    "The opening tag is not on the right line. This should always be the first line in a PHP file.",
                );
                violations.push(self.new_violation(file, suggestion, span));
            }

            if column > 0 {
                let suggestion = format!(
                    "The opening tag doesn't start at the right column: {}.",
                    column + 1
                );
                violations.push(self.new_violation(file, suggestion, span));
            }
        }

        violations
    }

    fn travers_statements_to_validate<'a>(
        &'a self,
        flatten_statements: &mut Vec<&'a Statement<'a>>,
        statement: &'a Statement<'a>,
    ) {
        if let Statement::OpeningTag(_) = statement {
            flatten_statements.push(statement);
        }
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

    #[test]
    fn bin_with_valid_openning_tag() {
        let violations = analyze_file_for_rule("e1/bin_with_valid_openning_tag.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
