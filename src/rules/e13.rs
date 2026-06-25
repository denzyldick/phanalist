use std::collections::{HashMap, HashSet};

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};

pub(crate) static CODE: &str = "E0013";
static DESCRIPTION: &str = "Private method not being called.";

pub struct Rule {}

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

        if let Statement::Class(class) = statement {
            // Collect all private method names and their spans
            let mut private_methods: HashMap<String, Span> = HashMap::new();
            // Collect all method names that are called anywhere in the class
            let mut called_methods: HashSet<String> = HashSet::new();

            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    let method_name = String::from_utf8_lossy(method.name.value).into_owned();
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
                                        expr_stmt.expression,
                                        &mut called_methods,
                                    );
                                }
                                if let Statement::Return(ret) = s {
                                    if let Some(value) = ret.value {
                                        self.collect_called_methods(value, &mut called_methods);
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
                    let message = Message::new(
                        "E0013:private-method-not-called",
                        "The private method {name} is not being called. ",
                    )
                    .arg("name", name.to_string());
                    violations.push(self.new_violation(file, message, *span));
                }
            }
        }

        violations
    }
}

impl Rule {
    fn collect_called_methods(&self, expr: &Expression<'_>, called: &mut HashSet<String>) {
        match expr {
            Expression::Call(Call::Method(m)) => {
                // $this->methodName()
                if let ClassLikeMemberSelector::Identifier(id) = &m.method {
                    called.insert(String::from_utf8_lossy(id.value).into_owned());
                }
                self.collect_called_methods(m.object, called);
                for arg in m.argument_list.arguments.iter() {
                    match arg {
                        Argument::Positional(a) => {
                            self.collect_called_methods(a.value, called);
                        }
                        Argument::Named(a) => {
                            self.collect_called_methods(a.value, called);
                        }
                    }
                }
            }
            Expression::Binary(bin) => {
                self.collect_called_methods(bin.lhs, called);
                self.collect_called_methods(bin.rhs, called);
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
