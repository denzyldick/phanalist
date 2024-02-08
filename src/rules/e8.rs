use php_parser_rs::parser::ast::{
    classes::ClassMember, functions::MethodBody, ReturnStatement, Statement,
};

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

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();
        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::ConcreteMethod(method) = member {
                    // Detect return statement without the proper return type signature.
                    let has_return = Self::method_has_return(method.body.clone());
                    let method_name = &method.name.value;

                    if let Some(ReturnStatement {
                        r#return,
                        value: _,
                        ending: _,
                    }) = has_return
                    {
                        if method.return_type.is_none() {
                            let suggestion = format!("The method {} has a return statement but it has no return type signature.", method_name).to_string();
                            violations.push(self.new_violation(file, suggestion, r#return));
                        };
                    };
                }
            }
        };
        violations
    }
}

impl Rule {
    fn method_has_return(body: MethodBody) -> Option<ReturnStatement> {
        let mut r: Option<ReturnStatement> = None;
        for statement in body.statements {
            if let Statement::Return(ReturnStatement {
                r#return,
                value,
                ending,
            }) = statement
            {
                r = Some(ReturnStatement {
                    r#return,
                    value,
                    ending,
                });
            };
        }
        r
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
