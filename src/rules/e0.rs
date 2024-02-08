use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0000";
static DESCRIPTION: &str = "Example rule";

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn validate(&self, _file: &File, _statement: &Statement) -> Vec<Violation> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e0/example.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
