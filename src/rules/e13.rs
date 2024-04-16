use php_parser_rs::lexer::byte_string::ByteString;
use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::functions::MethodBody;
use php_parser_rs::parser::ast::identifiers::Identifier;
use php_parser_rs::parser::ast::modifiers::MethodModifier::Static;
use php_parser_rs::parser::ast::modifiers::MethodModifierGroup;
use php_parser_rs::parser::ast::{
    comments, Expression, ExpressionStatement, MethodCallExpression, Statement,
};
use php_parser_rs::printer::print;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0013";
static DESCRIPTION: &str = "Unused private method.";

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
                    let mut is_private = false;
                    if let MethodModifierGroup { modifiers } = &method.modifiers {
                        for m in modifiers {
                            if let php_parser_rs::parser::ast::modifiers::MethodModifier::Private(
                                _,
                            ) = *m
                            {
                                is_private = true;
                                for modifier in modifiers {
                                    if let Static(_) = *modifier {
                                        is_private = true;
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
                                
                            }
                        }
                    }
                }
                if let ClassMember::ConcreteConstructor(constructor) = member {
                    let MethodBody {
                        statements,
                        comments,
                        left_brace,
                        right_brace,
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
