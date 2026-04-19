use mago_span::HasSpan;
use mago_syntax::ast::{ClassLikeMember, Statement};

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

pub struct Rule {}

const CODE: &str = "E0004";
const DESCRIPTION: &str = "Uppercase constants";

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

        match statement {
            Statement::Constant(c) => {
                for item in c.items.iter() {
                    let name = item.name.value;
                    if name != name.to_uppercase() {
                        let suggestion = format!("The constant {} should be uppercase.", name);
                        violations.push(self.new_violation(file, suggestion, item.name.span()));
                    }
                }
            }
            Statement::Class(c) => {
                collect_member_constants(self, file, &c.members, &mut violations)
            }
            Statement::Interface(i) => {
                collect_member_constants(self, file, &i.members, &mut violations)
            }
            Statement::Trait(t) => {
                collect_member_constants(self, file, &t.members, &mut violations)
            }
            Statement::Enum(e) => collect_member_constants(self, file, &e.members, &mut violations),
            _ => {}
        }

        violations
    }
}

fn collect_member_constants(
    rule: &Rule,
    file: &File<'_>,
    members: &mago_syntax::ast::Sequence<'_, ClassLikeMember<'_>>,
    violations: &mut Vec<Violation>,
) {
    for member in members.iter() {
        if let ClassLikeMember::Constant(const_member) = member {
            for item in const_member.items.iter() {
                let name = item.name.value;
                if name != name.to_uppercase() {
                    let suggestion = format!("The constant {} should be uppercase.", name);
                    violations.push(RuleTrait::new_violation(
                        rule,
                        file,
                        suggestion,
                        item.name.span(),
                    ));
                }
            }
        }
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
            "The constant TeST should be uppercase.".to_string()
        );
    }

    #[test]
    fn uppercase_constant() {
        let violations = analyze_file_for_rule("e4/uppercase_constant.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
