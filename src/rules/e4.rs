use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::constant::ConstantEntry;
use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0004";
static DESCRIPTION: &str = "Uppercase constants";

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

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::Constant(constant) = member {
                    for entry in &constant.entries {
                        if !Self::uppercased_constant_name(entry.clone()) {
                            let suggestion = format!(
                                "All letters in a constant({}) should be uppercase.",
                                entry.name.value
                            );
                            violations.push(self.new_violation(file, suggestion, entry.name.span))
                        }
                    }
                }
            }
        };

        violations
    }
}

impl Rule {
    fn uppercased_constant_name(entry: ConstantEntry) -> bool {
        let ConstantEntry {
            name,
            equals: _,
            value: _,
        } = entry;

        let mut is_uppercase = true;
        for l in name.value.to_string().chars() {
            if !l.is_uppercase() && l.is_alphabetic() {
                is_uppercase = l.is_uppercase()
            }
        }

        is_uppercase
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn no_uppercase_constant() {
        let violations = analyze_file_for_rule("e4/no_uppercase_constant.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "All letters in a constant(TeST) should be uppercase.".to_string()
        );
    }

    #[test]
    fn uppercase_constant() {
        let violations = analyze_file_for_rule("e4/uppercase_constant.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
