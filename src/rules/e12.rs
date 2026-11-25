use mago_span::HasSpan;
use mago_syntax::cst::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::walk_statement_expressions;
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0012";
static DESCRIPTION: &str = "Service compatibility with Shared Memory Model";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub include_namespaces: Vec<String>,
    pub exclude_namespaces: Vec<String>,
    pub reset_interfaces: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            include_namespaces: vec![
                "App\\Service\\".to_string(),
                "App\\Controller\\".to_string(),
            ],
            exclude_namespaces: vec![],
            reset_interfaces: vec!["ResetInterface".to_string()],
        }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl RuleTrait for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn settings_json(&self) -> Option<Value> {
        serde_json::to_value(&self.settings).ok()
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        };
    }

    fn do_validate(&self, file: &File<'_>) -> bool {
        if let Some(ns) = &file.namespace {
            return crate::rules::do_validate_namespace(
                ns.clone(),
                &self.settings.include_namespaces,
                &self.settings.exclude_namespaces,
            );
        }

        true
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            if self.implements_reset_interface(class) {
                return violations;
            }

            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    if method.name.value == b"__construct" {
                        continue;
                    }

                    if let MethodBody::Concrete(block) = &method.body {
                        for stmt in block.statements.iter() {
                            walk_statement_expressions(stmt, &mut |expression| {
                                self.check_expression(file, expression, &mut violations);
                            });
                        }
                    }
                }
            }
        }

        violations
    }
}

impl Rule {
    fn implements_reset_interface(&self, class: &Class<'_>) -> bool {
        if self.settings.reset_interfaces.is_empty() {
            return false;
        }

        if let Some(implements) = &class.implements {
            for interface in implements.types.iter() {
                let name = identifier_name(interface);
                for reset_interface in &self.settings.reset_interfaces {
                    if name.ends_with(reset_interface) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Inspects a single expression node; children are visited by the shared
    /// expression walker, so this must not recurse itself.
    fn check_expression(
        &self,
        file: &File<'_>,
        expression: &Expression<'_>,
        violations: &mut Vec<Violation>,
    ) {
        match expression {
            Expression::Assignment(assignment) => {
                self.check_assignment_lhs(file, assignment.lhs, violations);
            }
            Expression::UnaryPrefix(prefix)
                if matches!(
                    prefix.operator,
                    UnaryPrefixOperator::PreIncrement(_) | UnaryPrefixOperator::PreDecrement(_)
                ) =>
            {
                self.check_assignment_lhs(file, prefix.operand, violations);
            }
            Expression::UnaryPostfix(postfix) => {
                // UnaryPostfixOperator only has PostIncrement/PostDecrement, both of
                // which are assignment-producing, so any postfix reaches the LHS check.
                let _: &UnaryPostfixOperator = &postfix.operator;
                self.check_assignment_lhs(file, postfix.operand, violations);
            }
            _ => {}
        }
    }

    fn check_assignment_lhs(
        &self,
        file: &File<'_>,
        expression: &Expression<'_>,
        violations: &mut Vec<Violation>,
    ) {
        if let Expression::Access(access) = expression {
            let span = access.span();
            match access {
                Access::Property(prop) if is_this(prop.object) => {
                    violations.push(self.new_violation(
                        file,
                        Message::new(
                            "E0012:mutable-property",
                            "Properties in service must be immutable. Violating Shared Memory Model.",
                        ),
                        span,
                    ));
                }
                Access::StaticProperty(prop) if is_self_or_static_or_class(prop.class) => {
                    violations.push(self.new_violation(
                        file,
                        Message::new(
                            "E0012:mutable-static-property",
                            "Static properties in service must be immutable. Violating Shared Memory Model.",
                        ),
                        span,
                    ));
                }
                _ => {}
            }
        }
    }
}

fn identifier_name(identifier: &Identifier<'_>) -> String {
    String::from_utf8_lossy(identifier.value()).into_owned()
}

fn is_this(expression: &Expression<'_>) -> bool {
    if let Expression::Variable(Variable::Direct(direct)) = expression {
        return direct.name == b"$this";
    }
    false
}

fn is_self_or_static_or_class(class_id: &Expression<'_>) -> bool {
    match class_id {
        Expression::Self_(_) => true,
        Expression::Static(_) => true,
        Expression::Identifier(_) => {
            // In case of Foo::$prop, we might want to check if Foo is the class itself?
            // But generally static access on ANY class in a service is fishy if it modifies state.
            // However, the rule is "Service compatibility with Shared Memory Model".
            // Modifying static property of ANY class is bad?
            // Or only logic inside service?
            // Original rule likely targeted self/static.
            // But let's assume valid checks for now.
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_source_for_rule;

    use super::*;

    #[test]
    fn detects_property_assignments_in_nested_expressions() {
        let source = r#"<?php
final class Service {
    private $state;
    private static $shared;

    public function run(): void {
        if ($this->state = 1) {}
        $this->state ??= 2;
        self::$shared = 3;
        for ($i = 0; $i < 1; $this->state++) {}
    }
}
"#;

        let violations = analyze_source_for_rule(source, "e12-inline.php", CODE);
        assert_eq!(violations.len(), 4, "{violations:?}");
    }

    #[test]
    fn ignores_constructor_assignments() {
        let source = r#"<?php
final class Service {
    private $state;

    public function __construct() {
        $this->state = 1;
    }
}
"#;

        assert!(analyze_source_for_rule(source, "e12-inline.php", CODE).is_empty());
    }
}
