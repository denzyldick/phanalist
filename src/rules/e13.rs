use std::collections::{HashMap, HashSet};

use mago_ast::ast::class_like::member::ClassLikeMember;
use mago_ast::ast::class_like::method::MethodBody;
use mago_ast::ast::expression::Expression;
use mago_ast::ast::modifier::Modifier;
use mago_ast::ast::Statement;
use mago_ast::Call;
use mago_span::HasSpan;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0013";
static DESCRIPTION: &str = "Private method not being called.";

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
            // Collect all private method names and their spans
            let mut private_methods: HashMap<String, mago_span::Span> = HashMap::new();
            // Collect all method names that are called anywhere in the class
            let mut called_methods: HashSet<String> = HashSet::new();

            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    let method_name = file.interner.lookup(&method.name.value).to_string();
                    let is_private = method
                        .modifiers
                        .iter()
                        .any(|m| matches!(m, Modifier::Private(_)));

                    if is_private {
                        private_methods.insert(method_name, method.span());
                    }

                    // Scan method body for method calls
                    if let MethodBody::Concrete(block) = &method.body {
                        for stmt in block.statements.iter() {
                            let flat = self.flatten_statements_to_validate(stmt);
                            for s in flat {
                                if let Statement::Expression(expr_stmt) = s {
                                    self.collect_called_methods(
                                        file,
                                        &expr_stmt.expression,
                                        &mut called_methods,
                                    );
                                }
                                if let Statement::Return(ret) = s {
                                    if let Some(value) = &ret.value {
                                        self.collect_called_methods(
                                            file,
                                            value,
                                            &mut called_methods,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Report private methods that are never called
            for (name, span) in &private_methods {
                if !called_methods.contains(name) {
                    let message = format!("The private method {} is not being called. ", name);
                    violations.push(self.new_violation(file, message, *span));
                }
            }
        }

        violations
    }
}

impl Rule {
    fn collect_called_methods(&self, file: &File, expr: &Expression, called: &mut HashSet<String>) {
        match expr {
            Expression::Call(call) => {
                match call {
                    Call::Method(m) => {
                        // $this->methodName()
                        if let mago_ast::ClassLikeMemberSelector::Identifier(id) = &m.method {
                            let name = file.interner.lookup(&id.value).to_string();
                            called.insert(name);
                        }
                        self.collect_called_methods(file, &m.object, called);
                        for arg in m.argument_list.arguments.iter() {
                            match arg {
                                mago_ast::ast::argument::Argument::Positional(a) => {
                                    self.collect_called_methods(file, &a.value, called);
                                }
                                mago_ast::ast::argument::Argument::Named(a) => {
                                    self.collect_called_methods(file, &a.value, called);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Expression::Binary(bin) => {
                self.collect_called_methods(file, &bin.lhs, called);
                self.collect_called_methods(file, &bin.rhs, called);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e13/private_method_not_being_called.php", CODE);
        println!("{}", violations.len());
        assert!(violations.len().eq(&3));
    }
}
