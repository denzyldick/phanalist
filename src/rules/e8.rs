use php_parser_rs::parser::ast::{
    classes::ClassMember, functions::MethodBody, ReturnStatement, Statement,
};

use crate::file::File;
use crate::results::Violation;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0008")
    }

    fn description(&self) -> String {
        String::from("Return type signature")
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();
        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::ConcreteMethod(concretemethod) = member {
                    // Detect return statement without the proper return type signature.
                    let has_return = Self::method_has_return(concretemethod.body.clone());
                    let method_name = &concretemethod.name.value;

                    if let Some(ReturnStatement {
                        r#return,
                        value: _,
                        ending: _,
                    }) = has_return
                    {
                        if concretemethod.return_type.is_none() {
                            let suggestion = format!("The {} has a return statement but it has no return type signature.", method_name).to_string();
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
