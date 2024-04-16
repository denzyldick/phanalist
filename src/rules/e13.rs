use php_parser_rs::lexer::byte_string::ByteString;
use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::identifiers::{Identifier, SimpleIdentifier};
use php_parser_rs::parser::ast::MethodCallExpression;
use php_parser_rs::parser::ast::{
    functions::MethodBody,
    modifiers::{MethodModifier::Static, MethodModifierGroup},
    ExpressionStatement, Statement,
};
use std::collections::HashMap;
use std::ops::Add;
use std::rc::Rc;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0013";
static DESCRIPTION: &str = "Detect dead code.";

pub struct Rule {}

#[derive(Debug)]
struct RC {
    methods: HashMap<ByteString, isize>,
}
impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn validate(&self, _file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();
        let mut rc = RC {
            methods: HashMap::new(),
        };
        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::ConcreteMethod(method) = member {
                    let mut r = false;
                    if let MethodModifierGroup { modifiers } = &method.modifiers {
                        for modifier in modifiers {
                            if let php_parser_rs::parser::ast::modifiers::MethodModifier::Private(
                                _,
                            ) = *modifier
                            {
                                r = true;
                                for i in modifiers {
                                    if let Static(_) = *i {
                                        r = true;
                                    };
                                }
                            };
                        }
                    }
                    if r {
                        let scope_name = &method.name;
                        rc.methods
                            .insert(ByteString::from(scope_name.to_string()), 0);
                        let MethodBody { statements, .. } = &method.body;
                        for statement in statements {
                            if let Statement::Expression(ExpressionStatement {
                                expression,
                                ending: _,
                            }) = statement
                            {
                                if let php_parser_rs::parser::ast::Expression::MethodCall(call) =
                                    &expression
                                {
                                    if let php_parser_rs::parser::ast::Expression::Identifier(
                                        identifier,
                                    ) = *call.method.to_owned()
                                    {
                                        match identifier {
                                            Identifier::SimpleIdentifier(name) => {
                                                let method = ByteString::from(name.value);

                                                if let Some(entry) = rc.methods.get(&method.to_owned()) {

                                                    rc.methods.insert(method,entry.add(1));
                                                } else {
                                                };
                                            }
                                            Identifier::DynamicIdentifier(_) => {},
                                        };
                                    };
                                };
                            }
                        }
                    }
                }
                if let ClassMember::ConcreteConstructor(constructor) = member {
                    let MethodBody {
                        statements,
                        comments: _,
                        left_brace: _,
                        right_brace: _,
                    }: &MethodBody = &constructor.body;

                    let exists = statements.iter().filter(|statements| {
                        println!("{:#?}", statements);
                        rc.methods.insert("".into(), 0);

                        true
                    });
                }
            }
        }
        println!("{:#?}", rc);

        violations
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e13/detect_dead_code.php", CODE);

        assert!(violations.len().eq(&0));
        assert!(false);
    }
}
