pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0011")
    }

    fn validate(
        &self,
        _statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        vec![]
    }
}
