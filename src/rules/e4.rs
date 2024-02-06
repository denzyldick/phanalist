use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::constant::ConstantEntry;
use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0004")
    }

    fn description(&self) -> String {
        String::from("Uppercase constants")
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
