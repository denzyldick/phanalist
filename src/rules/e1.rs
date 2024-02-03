use php_parser_rs::parser::ast::Statement;

use crate::project::Suggestion;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0001")
    }

    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        match statement {
            Statement::FullOpeningTag(tag) => {
                let span = tag.span;
                if span.line > 1 {
                    return vec![
                        Suggestion::from(
                            "The opening tag <?php is not on the right line. This should always be the first line in a PHP file.".to_string(),
                            span,
                            self.get_code()
                        )
                    ];
                }

                if span.column > 1 {
                    return vec![Suggestion::from(
                        format!(
                            "The opening tag doesn't start at the right column: {}.",
                            span.column
                        )
                        .to_string(),
                        span,
                        self.get_code(),
                    )];
                }
            }
            Statement::ShortOpeningTag(tag) => {
                let span = tag.span;
                if span.line > 1 {
                    return vec![
                        Suggestion::from(
                            "The opening tag <?php is not on the right line. This should always be the first line in a PHP file.".to_string(),
                            span,
                            self.get_code()
                        )
                    ];
                }

                if span.column > 1 {
                    return vec![Suggestion::from(
                        format!(
                            "The opening tag doesn't start at the right column: {}.",
                            span.column
                        )
                        .to_string(),
                        span,
                        self.get_code(),
                    )];
                }
            }

            _ => {}
        };
        vec![]
    }
}
