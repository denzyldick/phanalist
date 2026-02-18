use crate::file::File;
use crate::results::Violation;
use mago_ast::ast::class_like::member::ClassLikeMember;
use mago_ast::Statement;
use mago_span::HasSpan;

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

    fn do_validate(&self, _file: &File) -> bool {
        true
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        match statement {
            Statement::Constant(c) => {
                for item in c.items.iter() {
                    let name = file.interner.lookup(&item.name.value);
                    if name != name.to_uppercase() {
                        let suggestion = format!("The constant {} should be uppercase.", name);
                        violations.push(self.new_violation(file, suggestion, item.name.span()));
                    }
                }
            }
            Statement::Class(c) => {
                for member in c.members.iter() {
                    if let ClassLikeMember::Constant(const_member) = member {
                        for item in const_member.items.iter() {
                            let name = file.interner.lookup(&item.name.value);
                            if name != name.to_uppercase() {
                                let suggestion =
                                    format!("The constant {} should be uppercase.", name);
                                violations.push(self.new_violation(
                                    file,
                                    suggestion,
                                    item.name.span(),
                                ));
                            }
                        }
                    }
                }
            }
            Statement::Interface(i) => {
                for member in i.members.iter() {
                    if let ClassLikeMember::Constant(const_member) = member {
                        for item in const_member.items.iter() {
                            let name = file.interner.lookup(&item.name.value);
                            if name != name.to_uppercase() {
                                let suggestion =
                                    format!("The constant {} should be uppercase.", name);
                                violations.push(self.new_violation(
                                    file,
                                    suggestion,
                                    item.name.span(),
                                ));
                            }
                        }
                    }
                }
            }
            Statement::Trait(t) => {
                for member in t.members.iter() {
                    if let ClassLikeMember::Constant(const_member) = member {
                        for item in const_member.items.iter() {
                            let name = file.interner.lookup(&item.name.value);
                            if name != name.to_uppercase() {
                                let suggestion =
                                    format!("The constant {} should be uppercase.", name);
                                violations.push(self.new_violation(
                                    file,
                                    suggestion,
                                    item.name.span(),
                                ));
                            }
                        }
                    }
                }
            }
            Statement::Enum(e) => {
                for member in e.members.iter() {
                    if let ClassLikeMember::Constant(const_member) = member {
                        for item in const_member.items.iter() {
                            let name = file.interner.lookup(&item.name.value);
                            if name != name.to_uppercase() {
                                let suggestion =
                                    format!("The constant {} should be uppercase.", name);
                                violations.push(self.new_violation(
                                    file,
                                    suggestion,
                                    item.name.span(),
                                ));
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        violations
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
