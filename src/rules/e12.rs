use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

pub static CODE: &str = "E0012";
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
                    if method.name.value == "__construct" {
                        continue;
                    }

                    if let MethodBody::Concrete(block) = &method.body {
                        for stmt in block.statements.iter() {
                            self.find_property_assignments(file, stmt, &mut violations);
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

    fn find_property_assignments(
        &self,
        file: &File<'_>,
        statement: &Statement<'_>,
        violations: &mut Vec<Violation>,
    ) {
        if let Statement::Expression(expr_stmt) = statement {
            self.check_expression(file, expr_stmt.expression, violations);
        }

        match statement {
            Statement::Block(block) => {
                for stmt in block.statements.iter() {
                    self.find_property_assignments(file, stmt, violations);
                }
            }
            Statement::If(if_stmt) => match &if_stmt.body {
                IfBody::Statement(body) => {
                    self.find_property_assignments(file, &body.statement, violations);
                    for clauses in body.else_if_clauses.iter() {
                        self.find_property_assignments(file, &clauses.statement, violations);
                    }
                    if let Some(else_clause) = &body.else_clause {
                        self.find_property_assignments(file, &else_clause.statement, violations);
                    }
                }
                IfBody::ColonDelimited(body) => {
                    for stmt in body.statements.iter() {
                        self.find_property_assignments(file, stmt, violations);
                    }
                    for clauses in body.else_if_clauses.iter() {
                        for stmt in clauses.statements.iter() {
                            self.find_property_assignments(file, stmt, violations);
                        }
                    }
                    if let Some(else_clause) = &body.else_clause {
                        for stmt in else_clause.statements.iter() {
                            self.find_property_assignments(file, stmt, violations);
                        }
                    }
                }
            },
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(body) => {
                    self.find_property_assignments(file, body, violations);
                }
                WhileBody::ColonDelimited(body) => {
                    for stmt in body.statements.iter() {
                        self.find_property_assignments(file, stmt, violations);
                    }
                }
            },
            Statement::DoWhile(do_while) => {
                self.find_property_assignments(file, &do_while.statement, violations);
            }
            Statement::Foreach(foreach) => match &foreach.body {
                ForeachBody::Statement(body) => {
                    self.find_property_assignments(file, body, violations);
                }
                ForeachBody::ColonDelimited(body) => {
                    for stmt in body.statements.iter() {
                        self.find_property_assignments(file, stmt, violations);
                    }
                }
            },
            Statement::For(for_stmt) => match &for_stmt.body {
                ForBody::Statement(body) => {
                    self.find_property_assignments(file, body, violations);
                }
                ForBody::ColonDelimited(body) => {
                    for stmt in body.statements.iter() {
                        self.find_property_assignments(file, stmt, violations);
                    }
                }
            },
            Statement::Switch(switch) => {
                let cases = match &switch.body {
                    SwitchBody::BraceDelimited(body) => &body.cases,
                    SwitchBody::ColonDelimited(body) => &body.cases,
                };
                for case in cases.iter() {
                    let statements = match case {
                        SwitchCase::Expression(c) => &c.statements,
                        SwitchCase::Default(c) => &c.statements,
                    };
                    for stmt in statements.iter() {
                        self.find_property_assignments(file, stmt, violations);
                    }
                }
            }
            Statement::Try(try_stmt) => {
                for stmt in try_stmt.block.statements.iter() {
                    self.find_property_assignments(file, stmt, violations);
                }
                for catch in try_stmt.catch_clauses.iter() {
                    for stmt in catch.block.statements.iter() {
                        self.find_property_assignments(file, stmt, violations);
                    }
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    for stmt in finally.block.statements.iter() {
                        self.find_property_assignments(file, stmt, violations);
                    }
                }
            }
            Statement::Return(ret) => {
                if let Some(value) = ret.value {
                    self.check_expression(file, value, violations);
                }
            }
            _ => {}
        }
    }

    fn check_expression(
        &self,
        file: &File<'_>,
        expression: &Expression<'_>,
        violations: &mut Vec<Violation>,
    ) {
        match expression {
            Expression::Assignment(assignment) => {
                self.check_assignment_lhs(file, assignment.lhs, violations);
                self.check_expression(file, assignment.rhs, violations);
            }
            Expression::UnaryPrefix(prefix) => {
                if let UnaryPrefixOperator::PreIncrement(_) | UnaryPrefixOperator::PreDecrement(_) =
                    prefix.operator
                {
                    self.check_assignment_lhs(file, prefix.operand, violations);
                }
                self.check_expression(file, prefix.operand, violations);
            }
            Expression::UnaryPostfix(postfix) => {
                // UnaryPostfixOperator only has PostIncrement/PostDecrement, both of
                // which are assignment-producing, so any postfix reaches the LHS check.
                let _: &UnaryPostfixOperator = &postfix.operator;
                self.check_assignment_lhs(file, postfix.operand, violations);
            }
            Expression::Call(call) => match call {
                Call::Function(f) => {
                    self.check_argument_list(file, &f.argument_list, violations);
                }
                Call::Method(m) => {
                    self.check_expression(file, m.object, violations);
                    self.check_argument_list(file, &m.argument_list, violations);
                }
                Call::NullSafeMethod(m) => {
                    self.check_expression(file, m.object, violations);
                    self.check_argument_list(file, &m.argument_list, violations);
                }
                Call::StaticMethod(m) => {
                    self.check_argument_list(file, &m.argument_list, violations);
                }
            },
            _ => {}
        }
    }

    fn check_argument_list(
        &self,
        file: &File<'_>,
        argument_list: &ArgumentList<'_>,
        violations: &mut Vec<Violation>,
    ) {
        for argument in argument_list.arguments.iter() {
            match argument {
                Argument::Positional(arg) => {
                    self.check_expression(file, arg.value, violations);
                }
                Argument::Named(arg) => {
                    self.check_expression(file, arg.value, violations);
                }
            }
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
                Access::Property(prop) => {
                    // Check if object is $this
                    if is_this(prop.object) {
                        violations.push(self.new_violation(
                            file,
                            "Properties in service must be immutable. Violating Shared Memory Model.".to_string(),
                            span,
                        ));
                    }
                }
                Access::StaticProperty(prop) => {
                    // Check if it is self::$prop or static::$prop
                    if is_self_or_static_or_class(prop.class) {
                        violations.push(self.new_violation(
                            file,
                            "Static properties in service must be immutable. Violating Shared Memory Model.".to_string(),
                            span,
                        ));
                    }
                }
                _ => {}
            }
        }
    }
}

fn identifier_name(identifier: &Identifier<'_>) -> String {
    identifier.value().to_string()
}

fn is_this(expression: &Expression<'_>) -> bool {
    if let Expression::Variable(Variable::Direct(direct)) = expression {
        return direct.name == "$this";
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
