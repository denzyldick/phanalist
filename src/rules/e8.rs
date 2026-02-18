use mago_ast::ast::class_like::member::ClassLikeMember;
use mago_ast::ast::class_like::method::MethodBody;
use mago_ast::ast::Statement;
use mago_span::HasSpan;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0008";
static DESCRIPTION: &str = "Return type signature";

pub struct Rule {}

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

        if let Statement::Class(class) = statement {
            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    // Only check concrete methods (not abstract)
                    if let MethodBody::Concrete(block) = &method.body {
                        // Check if method has a return statement
                        let has_return = block
                            .statements
                            .iter()
                            .any(|s| matches!(s, Statement::Return(_)));

                        if has_return && method.return_type_hint.is_none() {
                            let method_name = file.interner.lookup(&method.name.value);
                            let suggestion = format!(
                                "The method {} has a return statement but it has no return type signature.",
                                method_name
                            );
                            violations.push(self.new_violation(file, suggestion, method.span()));
                        }
                    }
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn no_return_type() {
        let violations = analyze_file_for_rule("e8/no_return_type.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "The method test has a return statement but it has no return type signature."
                .to_string()
        );
    }

    #[test]
    fn valid_amount_of_params() {
        let violations = analyze_file_for_rule("e8/with_return_type.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
