use php_parser_rs::parser::ast::{
    classes::ClassMember, functions::MethodBody, ReturnStatement, Statement,
};

use crate::{analyse::Rule, project::Suggestion};

pub struct E008 {}

impl Rule for E008 {
    fn validate(
        &self,
        statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        let mut suggestions = Vec::new();
        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::ConcreteMethod(concretemethod) = member {
                    // Detect return statement without the proper return type signature.
                    let has_return = method_has_return(concretemethod.body.clone());
                    let method_name = &concretemethod.name.value;

                    if let Some(ReturnStatement {
                        r#return,
                        value: _,
                        ending: _,
                    }) = has_return {
                        if concretemethod.return_type.is_none() {
                                suggestions.push(
                                                    Suggestion::from(
                                                        format!("The {} has a return statement but it has no return type signature.", method_name).to_string(),
                                                    r#return,
                                            "E008".to_string(),

                                                    )
                                                );
                        };
                    };
                }
            }
        };
        suggestions
    }
}
/// Return the type of method body.  
fn method_has_return(body: MethodBody) -> Option<ReturnStatement> {
    let mut r: Option<ReturnStatement> = None;
    for statement in body.statements {
        if let Statement::Return(ReturnStatement {
            r#return,
            value,
            ending,
        }) = statement {
            r = Some(ReturnStatement {
                r#return,
                value,
                ending,
            });
        };
    }
    r
}
