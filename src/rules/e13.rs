use php_parser_rs::lexer::byte_string::ByteString;
use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::identifiers::Identifier;

use php_parser_rs::parser::ast::{
    functions::MethodBody,
    modifiers::{MethodModifier::Static, MethodModifierGroup},
    ExpressionStatement, Statement,
};
use std::collections::HashMap;
use std::ops::Add;

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
        let violations = Vec::new();
        dbg!("HERE");
        let mut rc = RC {
            methods: HashMap::new(),
        };
        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::ConcreteMethod(method) = member {
                    let mut r = false;
                    if let MethodModifierGroup { modifiers } = &method.modifiers {
                        for modifier in modifiers {
                            Self::skip_if_public(modifier, &mut r, modifiers);
                        }
                    }
                    if r {
                        Self::create_reference(method, &mut rc);
                        let MethodBody { statements, .. } = &method.body;
                        for statement in statements {
                            let Statement::Expression(ExpressionStatement {
                                expression,
                                ending: _,
                            }) = statement
                            else {
                                continue;
                            };
                            if let php_parser_rs::parser::ast::Expression::MethodCall(call) =
                                &expression
                            {
                                if let php_parser_rs::parser::ast::Expression::Identifier(
                                    identifier,
                                ) = *call.method.to_owned()
                                {
                                    match identifier {
                                        Identifier::SimpleIdentifier(name) => {
                                            let method = name.value;

                                            dbg!(&method);
                                            match rc.methods.get(&method.to_owned()) {
                                                Some(entry) => {
                                                    let n = entry.add(1);
                                                    dbg!(&n);
                                                    rc.methods.insert(method, n);
                                                }
                                                None => {
                                                    rc.methods.insert(method, 1);
                                                }
                                            };
                                        }
                                        Identifier::DynamicIdentifier(dynamic_identifier) => {
                                            dbg!(dynamic_identifier);
                                        }
                                    };
                                };
                            };
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

                    let _exists = statements.iter().filter(|statements| {
                        println!("{:#?}", statements);
                        rc.methods.insert("".into(), 0);

                        true
                    });
                }
            }
        }
        dbg!(rc);

        violations
    }
}

impl Rule {
    fn create_reference(method: &php_parser_rs::parser::ast::functions::ConcreteMethod, rc: &mut RC) {
        let scope_name = &method.name;
        rc.methods
            .insert(ByteString::from(scope_name.to_string()), 0);
    }
    fn skip_if_public(
        modifier: &php_parser_rs::parser::ast::modifiers::MethodModifier,
        r: &mut bool,
        modifiers: &Vec<php_parser_rs::parser::ast::modifiers::MethodModifier>,
    ) {
        if let php_parser_rs::parser::ast::modifiers::MethodModifier::Private(_) = *modifier {
            *r = true;
            for i in modifiers {
                match *i {
                    Static(_) => {
                        *r = true;
                    }
                    php_parser_rs::parser::ast::modifiers::MethodModifier::Private(_) => {
                        *r = true;
                    }
                    _ => {}
                };
            }
        };
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
