use std::collections::HashSet;

use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0019";
static DESCRIPTION: &str = "Response For a Class (RFC)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_rfc: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_rfc: 50 }
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

    fn do_validate(&self, _file: &File<'_>) -> bool {
        true
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        }
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            let mut own_method_count: usize = 0;
            let mut called_methods: HashSet<String> = HashSet::new();

            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    own_method_count += 1;

                    if let MethodBody::Concrete(block) = &method.body {
                        for stmt in block.statements.iter() {
                            self.scan_statement(stmt, &mut called_methods);
                        }
                    }
                }
            }

            let rfc = own_method_count + called_methods.len();

            if rfc > self.settings.max_rfc {
                let message = Message::new(
                    "E0019:high-rfc",
                    "Class \"{class}\" has a Response For Class (RFC) of {rfc} (threshold: {threshold}). The class responds to too many messages.",
                )
                .arg("class", String::from_utf8_lossy(class.name.value).into_owned())
                .arg("rfc", rfc.to_string())
                .arg("threshold", self.settings.max_rfc.to_string());
                violations.push(self.new_violation(file, message, class.span()));
            }
        }

        violations
    }
}

impl Rule {
    fn scan_statement(&self, statement: &Statement<'_>, called_methods: &mut HashSet<String>) {
        match statement {
            Statement::Expression(expr) => {
                self.scan_expression(expr.expression, called_methods);
            }
            Statement::Return(ret) => {
                if let Some(value) = ret.value {
                    self.scan_expression(value, called_methods);
                }
            }
            Statement::Echo(echo) => {
                for value in echo.values.iter() {
                    self.scan_expression(value, called_methods);
                }
            }
            Statement::Block(block) => {
                for s in block.statements.iter() {
                    self.scan_statement(s, called_methods);
                }
            }
            Statement::If(if_stmt) => {
                self.scan_expression(if_stmt.condition, called_methods);
                match &if_stmt.body {
                    IfBody::Statement(body) => {
                        self.scan_statement(body.statement, called_methods);
                        for clause in body.else_if_clauses.iter() {
                            self.scan_expression(clause.condition, called_methods);
                            self.scan_statement(clause.statement, called_methods);
                        }
                        if let Some(else_clause) = &body.else_clause {
                            self.scan_statement(else_clause.statement, called_methods);
                        }
                    }
                    IfBody::ColonDelimited(body) => {
                        for s in body.statements.iter() {
                            self.scan_statement(s, called_methods);
                        }
                        for clause in body.else_if_clauses.iter() {
                            self.scan_expression(clause.condition, called_methods);
                            for s in clause.statements.iter() {
                                self.scan_statement(s, called_methods);
                            }
                        }
                        if let Some(else_clause) = &body.else_clause {
                            for s in else_clause.statements.iter() {
                                self.scan_statement(s, called_methods);
                            }
                        }
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.scan_expression(while_stmt.condition, called_methods);
                match &while_stmt.body {
                    WhileBody::Statement(body) => {
                        self.scan_statement(body, called_methods);
                    }
                    WhileBody::ColonDelimited(body) => {
                        for s in body.statements.iter() {
                            self.scan_statement(s, called_methods);
                        }
                    }
                }
            }
            Statement::DoWhile(do_while) => {
                self.scan_statement(do_while.statement, called_methods);
                self.scan_expression(do_while.condition, called_methods);
            }
            Statement::For(for_stmt) => {
                for init in for_stmt.initializations.iter() {
                    self.scan_expression(init, called_methods);
                }
                for cond in for_stmt.conditions.iter() {
                    self.scan_expression(cond, called_methods);
                }
                for inc in for_stmt.increments.iter() {
                    self.scan_expression(inc, called_methods);
                }
                match &for_stmt.body {
                    ForBody::Statement(body) => {
                        self.scan_statement(body, called_methods);
                    }
                    ForBody::ColonDelimited(body) => {
                        for s in body.statements.iter() {
                            self.scan_statement(s, called_methods);
                        }
                    }
                }
            }
            Statement::Foreach(foreach_stmt) => {
                self.scan_expression(foreach_stmt.expression, called_methods);
                match &foreach_stmt.body {
                    ForeachBody::Statement(body) => {
                        self.scan_statement(body, called_methods);
                    }
                    ForeachBody::ColonDelimited(body) => {
                        for s in body.statements.iter() {
                            self.scan_statement(s, called_methods);
                        }
                    }
                }
            }
            Statement::Try(try_stmt) => {
                for s in try_stmt.block.statements.iter() {
                    self.scan_statement(s, called_methods);
                }
                for catch in try_stmt.catch_clauses.iter() {
                    for s in catch.block.statements.iter() {
                        self.scan_statement(s, called_methods);
                    }
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    for s in finally.block.statements.iter() {
                        self.scan_statement(s, called_methods);
                    }
                }
            }
            _ => {}
        }
    }

    fn scan_expression(&self, expression: &Expression<'_>, called_methods: &mut HashSet<String>) {
        match expression {
            Expression::Call(call) => match call {
            Call::Method(method) => {
                if let ClassLikeMemberSelector::Identifier(id) = &method.method {
                    called_methods.insert(String::from_utf8_lossy(id.value).into_owned());
                }
                self.scan_expression(method.object, called_methods);
                for arg in method.argument_list.arguments.iter() {
                    self.scan_expression(arg.value(), called_methods);
                }
            }
            Call::NullSafeMethod(method) => {
                if let ClassLikeMemberSelector::Identifier(id) = &method.method {
                    called_methods.insert(String::from_utf8_lossy(id.value).into_owned());
                }
                self.scan_expression(method.object, called_methods);
                for arg in method.argument_list.arguments.iter() {
                    self.scan_expression(arg.value(), called_methods);
                }
            }
                Call::StaticMethod(method) => {
                    if let ClassLikeMemberSelector::Identifier(id) = &method.method {
                        let class_name = self.identifier_name(method.class);
                        let call_name = format!("{}::{}", class_name, String::from_utf8_lossy(id.value));
                        called_methods.insert(call_name);
                    }
                    for arg in method.argument_list.arguments.iter() {
                        self.scan_expression(arg.value(), called_methods);
                    }
                }
            Call::Function(func) => {
                if let Expression::Identifier(id) = func.function {
                    called_methods.insert(String::from_utf8_lossy(id.value()).into_owned());
                }
                    for arg in func.argument_list.arguments.iter() {
                        self.scan_expression(arg.value(), called_methods);
                    }
                }
            },
            Expression::Binary(binary) => {
                self.scan_expression(binary.lhs, called_methods);
                self.scan_expression(binary.rhs, called_methods);
            }
            Expression::UnaryPrefix(unary) => {
                self.scan_expression(unary.operand, called_methods);
            }
            Expression::UnaryPostfix(unary) => {
                self.scan_expression(unary.operand, called_methods);
            }
            Expression::Assignment(assignment) => {
                self.scan_expression(assignment.lhs, called_methods);
                self.scan_expression(assignment.rhs, called_methods);
            }
            Expression::Parenthesized(parenthesized) => {
                self.scan_expression(parenthesized.expression, called_methods);
            }
            Expression::Instantiation(inst) => {
                if let Some(args) = &inst.argument_list {
                    for arg in args.arguments.iter() {
                        self.scan_expression(arg.value(), called_methods);
                    }
                }
            }
            Expression::Closure(closure) => {
                for stmt in closure.body.statements.iter() {
                    self.scan_statement(stmt, called_methods);
                }
            }
            Expression::ArrowFunction(arrow) => {
                self.scan_expression(arrow.expression, called_methods);
            }
            Expression::Array(array) => {
                for element in array.elements.iter() {
                    if let Some(val) = element.get_value() {
                        self.scan_expression(val, called_methods);
                    }
                }
            }
            Expression::Conditional(cond) => {
                self.scan_expression(cond.condition, called_methods);
                if let Some(then) = cond.then {
                    self.scan_expression(then, called_methods);
                }
                self.scan_expression(cond.r#else, called_methods);
            }
            Expression::Match(match_expr) => {
                self.scan_expression(match_expr.expression, called_methods);
                for arm in match_expr.arms.iter() {
                    match arm {
                        MatchArm::Expression(a) => {
                            for cond in a.conditions.iter() {
                                self.scan_expression(cond, called_methods);
                            }
                            self.scan_expression(a.expression, called_methods);
                        }
                        MatchArm::Default(a) => {
                            self.scan_expression(a.expression, called_methods);
                        }
                    }
                }
            }
            Expression::Access(access) => match access {
                Access::Property(p) => {
                    self.scan_expression(p.object, called_methods);
                }
                Access::NullSafeProperty(p) => {
                    self.scan_expression(p.object, called_methods);
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn identifier_name(&self, expr: &Expression<'_>) -> String {
        match expr {
            Expression::Identifier(id) => String::from_utf8_lossy(id.value()).into_owned(),
            _ => "unknown".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn high_rfc() {
        let violations = analyze_file_for_rule("e19/high_rfc.php", CODE);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.render().contains("Response For Class (RFC)"));
    }

    #[test]
    fn low_rfc() {
        let violations = analyze_file_for_rule("e19/low_rfc.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn repeated_calls_counted_once() {
        let violations = analyze_file_for_rule("e19/repeated_calls.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn abstract_class() {
        let violations = analyze_file_for_rule("e19/abstract_class.php", CODE);
        assert_eq!(violations.len(), 0);
    }
}
