use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

pub static CODE: &str = "E0029";
static DESCRIPTION: &str = "Class-level Fan-in / Fan-out";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_fan_out: usize,
    pub max_fan_in: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_fan_out: 10,
            max_fan_in: 20,
        }
    }
}

#[derive(Default)]
struct ClassIndex {
    class_dependencies: HashMap<String, HashSet<String>>,
}

pub struct Rule {
    pub settings: Settings,
    index: Mutex<ClassIndex>,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            index: Mutex::new(ClassIndex::default()),
        }
    }
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

    fn index_file(&self, file: &File<'_>) {
        if let Some(program) = file.ast {
            for statement in program.statements.iter() {
                self.collect_class_deps(statement);
            }
        }
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            let class_name = class.name.value.to_string();
            let index = match self.index.lock() {
                Ok(idx) => idx,
                Err(_) => return violations,
            };

            let (fan_out, fan_in) = self.compute_fan_in_out(&class_name, &index);

            if fan_out > self.settings.max_fan_out {
                let suggestion = format!(
                    "Class \"{}\" has fan-out of {} (max: {}). It depends on too many other classes — consider reducing dependencies.",
                    class_name, fan_out, self.settings.max_fan_out
                );
                violations.push(self.new_violation(file, suggestion, class.span()));
            }

            if fan_in > self.settings.max_fan_in {
                let suggestion = format!(
                    "Class \"{}\" has fan-in of {} (max: {}). Too many classes depend on it — consider splitting or stabilizing its interface.",
                    class_name, fan_in, self.settings.max_fan_in
                );
                violations.push(self.new_violation(file, suggestion, class.span()));
            }
        }

        violations
    }
}

impl Rule {
    fn collect_class_deps(&self, statement: &Statement<'_>) {
        match statement {
            Statement::Namespace(ns) => {
                for s in ns.statements().iter() {
                    self.collect_class_deps(s);
                }
            }
            Statement::Class(class) => {
                let class_name = class.name.value.to_string();
                let mut deps = HashSet::new();

                if let Some(extends) = &class.extends {
                    for parent in extends.types.iter() {
                        deps.insert(self.normalize_name(parent.value()));
                    }
                }

                if let Some(implements) = &class.implements {
                    for iface in implements.types.iter() {
                        deps.insert(self.normalize_name(iface.value()));
                    }
                }

                for member in class.members.iter() {
                    self.collect_member_deps(member, &mut deps);
                }

                if let Ok(mut index) = self.index.lock() {
                    index.class_dependencies.insert(class_name, deps);
                }
            }
            _ => {}
        }
    }

    fn collect_member_deps(&self, member: &ClassLikeMember<'_>, deps: &mut HashSet<String>) {
        match member {
            ClassLikeMember::Method(method) => {
                for param in method.parameter_list.parameters.iter() {
                    if let Some(hint) = &param.hint {
                        self.collect_hint_deps(hint, deps);
                    }
                }
                if let Some(ret) = &method.return_type_hint {
                    self.collect_hint_deps(&ret.hint, deps);
                }
                if let MethodBody::Concrete(block) = &method.body {
                    for statement in block.statements.iter() {
                        self.scan_statement(statement, deps);
                    }
                }
            }
            ClassLikeMember::Property(property) => {
                if let Some(hint) = property.hint() {
                    self.collect_hint_deps(hint, deps);
                }
            }
            ClassLikeMember::TraitUse(trait_use) => {
                for trait_name in trait_use.trait_names.iter() {
                    deps.insert(self.normalize_name(trait_name.value()));
                }
            }
            _ => {}
        }
    }

    fn collect_hint_deps(&self, hint: &Hint<'_>, deps: &mut HashSet<String>) {
        match hint {
            Hint::Identifier(id) => {
                let name = self.normalize_name(id.value());
                if !self.is_builtin(&name) {
                    deps.insert(name);
                }
            }
            Hint::Nullable(n) => self.collect_hint_deps(n.hint, deps),
            Hint::Union(u) => {
                self.collect_hint_deps(u.left, deps);
                self.collect_hint_deps(u.right, deps);
            }
            Hint::Intersection(i) => {
                self.collect_hint_deps(i.left, deps);
                self.collect_hint_deps(i.right, deps);
            }
            Hint::Parenthesized(p) => self.collect_hint_deps(p.hint, deps),
            _ => {}
        }
    }

    fn scan_statement(&self, statement: &Statement<'_>, deps: &mut HashSet<String>) {
        match statement {
            Statement::Expression(expr) => {
                self.scan_expression(expr.expression, deps);
            }
            Statement::Return(ret) => {
                if let Some(value) = ret.value {
                    self.scan_expression(value, deps);
                }
            }
            Statement::Echo(echo) => {
                for value in echo.values.iter() {
                    self.scan_expression(value, deps);
                }
            }
            Statement::Block(block) => {
                for s in block.statements.iter() {
                    self.scan_statement(s, deps);
                }
            }
            Statement::If(if_stmt) => {
                self.scan_expression(if_stmt.condition, deps);
                match &if_stmt.body {
                    IfBody::Statement(body) => {
                        self.scan_statement(body.statement, deps);
                        for clause in body.else_if_clauses.iter() {
                            self.scan_expression(clause.condition, deps);
                            self.scan_statement(clause.statement, deps);
                        }
                        if let Some(else_clause) = &body.else_clause {
                            self.scan_statement(else_clause.statement, deps);
                        }
                    }
                    IfBody::ColonDelimited(body) => {
                        for s in body.statements.iter() {
                            self.scan_statement(s, deps);
                        }
                        for clause in body.else_if_clauses.iter() {
                            self.scan_expression(clause.condition, deps);
                            for s in clause.statements.iter() {
                                self.scan_statement(s, deps);
                            }
                        }
                        if let Some(else_clause) = &body.else_clause {
                            for s in else_clause.statements.iter() {
                                self.scan_statement(s, deps);
                            }
                        }
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.scan_expression(while_stmt.condition, deps);
                match &while_stmt.body {
                    WhileBody::Statement(body) => self.scan_statement(body, deps),
                    WhileBody::ColonDelimited(body) => {
                        for s in body.statements.iter() { self.scan_statement(s, deps); }
                    }
                }
            }
            Statement::DoWhile(do_while) => {
                self.scan_statement(do_while.statement, deps);
                self.scan_expression(do_while.condition, deps);
            }
            Statement::For(for_stmt) => {
                for init in for_stmt.initializations.iter() { self.scan_expression(init, deps); }
                for cond in for_stmt.conditions.iter() { self.scan_expression(cond, deps); }
                for inc in for_stmt.increments.iter() { self.scan_expression(inc, deps); }
                match &for_stmt.body {
                    ForBody::Statement(body) => self.scan_statement(body, deps),
                    ForBody::ColonDelimited(body) => {
                        for s in body.statements.iter() { self.scan_statement(s, deps); }
                    }
                }
            }
            Statement::Foreach(foreach) => {
                self.scan_expression(foreach.expression, deps);
                match &foreach.target {
                    ForeachTarget::Value(t) => self.scan_expression(t.value, deps),
                    ForeachTarget::KeyValue(t) => {
                        self.scan_expression(t.key, deps);
                        self.scan_expression(t.value, deps);
                    }
                }
                match &foreach.body {
                    ForeachBody::Statement(body) => self.scan_statement(body, deps),
                    ForeachBody::ColonDelimited(body) => {
                        for s in body.statements.iter() { self.scan_statement(s, deps); }
                    }
                }
            }
            Statement::Switch(switch) => {
                self.scan_expression(switch.expression, deps);
                let cases = match &switch.body {
                    SwitchBody::BraceDelimited(body) => &body.cases,
                    SwitchBody::ColonDelimited(body) => &body.cases,
                };
                for case in cases.iter() {
                    match case {
                        SwitchCase::Expression(c) => {
                            self.scan_expression(c.expression, deps);
                            for s in c.statements.iter() { self.scan_statement(s, deps); }
                        }
                        SwitchCase::Default(c) => {
                            for s in c.statements.iter() { self.scan_statement(s, deps); }
                        }
                    }
                }
            }
            Statement::Try(try_stmt) => {
                for s in try_stmt.block.statements.iter() { self.scan_statement(s, deps); }
                for catch in try_stmt.catch_clauses.iter() {
                    self.collect_hint_deps(&catch.hint, deps);
                    for s in catch.block.statements.iter() { self.scan_statement(s, deps); }
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    for s in finally.block.statements.iter() { self.scan_statement(s, deps); }
                }
            }
            _ => {}
        }
    }

    fn scan_expression(&self, expression: &Expression<'_>, deps: &mut HashSet<String>) {
        match expression {
            Expression::Instantiation(instantiation) => {
                self.scan_class_expression(instantiation.class, deps);
                if let Some(arguments) = &instantiation.argument_list {
                    for argument in arguments.arguments.iter() {
                        self.scan_argument(argument, deps);
                    }
                }
            }
            Expression::Call(call) => match call {
                Call::StaticMethod(method) => {
                    self.scan_class_expression(method.class, deps);
                    for argument in method.argument_list.arguments.iter() {
                        self.scan_argument(argument, deps);
                    }
                }
                Call::Method(method) => {
                    self.scan_expression(method.object, deps);
                    for argument in method.argument_list.arguments.iter() {
                        self.scan_argument(argument, deps);
                    }
                }
                Call::NullSafeMethod(method) => {
                    self.scan_expression(method.object, deps);
                    for argument in method.argument_list.arguments.iter() {
                        self.scan_argument(argument, deps);
                    }
                }
                Call::Function(function) => {
                    for argument in function.argument_list.arguments.iter() {
                        self.scan_argument(argument, deps);
                    }
                }
            },
            Expression::Access(access) => match access {
                Access::StaticProperty(property) => {
                    self.scan_class_expression(property.class, deps);
                }
                Access::ClassConstant(constant) => {
                    self.scan_class_expression(constant.class, deps);
                }
                Access::Property(property) => {
                    self.scan_expression(property.object, deps);
                }
                Access::NullSafeProperty(property) => {
                    self.scan_expression(property.object, deps);
                }
            },
            Expression::Binary(binary) => {
                self.scan_expression(binary.lhs, deps);
                if matches!(binary.operator, BinaryOperator::Instanceof(_)) {
                    self.scan_class_expression(binary.rhs, deps);
                } else {
                    self.scan_expression(binary.rhs, deps);
                }
            }
            Expression::UnaryPrefix(unary) => {
                self.scan_expression(unary.operand, deps);
            }
            Expression::UnaryPostfix(unary) => {
                self.scan_expression(unary.operand, deps);
            }
            Expression::Assignment(assignment) => {
                self.scan_expression(assignment.lhs, deps);
                self.scan_expression(assignment.rhs, deps);
            }
            Expression::Parenthesized(parenthesized) => {
                self.scan_expression(parenthesized.expression, deps);
            }
            Expression::Array(array) => {
                for element in array.elements.iter() {
                    self.scan_array_element(element, deps);
                }
            }
            Expression::LegacyArray(array) => {
                for element in array.elements.iter() {
                    self.scan_array_element(element, deps);
                }
            }
            Expression::List(list) => {
                for element in list.elements.iter() {
                    self.scan_array_element(element, deps);
                }
            }
            Expression::ArrayAccess(array_access) => {
                self.scan_expression(array_access.array, deps);
                self.scan_expression(array_access.index, deps);
            }
            Expression::ArrayAppend(array_append) => {
                self.scan_expression(array_append.array, deps);
            }
            Expression::Closure(closure) => {
                for param in closure.parameter_list.parameters.iter() {
                    if let Some(hint) = &param.hint {
                        self.collect_hint_deps(hint, deps);
                    }
                }
                if let Some(return_hint) = &closure.return_type_hint {
                    self.collect_hint_deps(&return_hint.hint, deps);
                }
                for s in closure.body.statements.iter() {
                    self.scan_statement(s, deps);
                }
            }
            Expression::ArrowFunction(arrow) => {
                for param in arrow.parameter_list.parameters.iter() {
                    if let Some(hint) = &param.hint {
                        self.collect_hint_deps(hint, deps);
                    }
                }
                if let Some(return_hint) = &arrow.return_type_hint {
                    self.collect_hint_deps(&return_hint.hint, deps);
                }
                self.scan_expression(arrow.expression, deps);
            }
            Expression::Conditional(conditional) => {
                self.scan_expression(conditional.condition, deps);
                if let Some(then_expr) = conditional.then {
                    self.scan_expression(then_expr, deps);
                }
                self.scan_expression(conditional.r#else, deps);
            }
            Expression::Match(match_expr) => {
                self.scan_expression(match_expr.expression, deps);
                for arm in match_expr.arms.iter() {
                    match arm {
                        MatchArm::Expression(arm) => {
                            for condition in arm.conditions.iter() {
                                self.scan_expression(condition, deps);
                            }
                            self.scan_expression(arm.expression, deps);
                        }
                        MatchArm::Default(arm) => {
                            self.scan_expression(arm.expression, deps);
                        }
                    }
                }
            }
            Expression::Throw(throw) => {
                self.scan_expression(throw.exception, deps);
            }
            Expression::Clone(clone) => {
                self.scan_expression(clone.object, deps);
            }
            Expression::Yield(yield_expr) => match yield_expr {
                Yield::Value(value) => {
                    if let Some(value) = value.value {
                        self.scan_expression(value, deps);
                    }
                }
                Yield::Pair(pair) => {
                    self.scan_expression(pair.key, deps);
                    self.scan_expression(pair.value, deps);
                }
                Yield::From(from) => {
                    self.scan_expression(from.iterator, deps);
                }
            },
            Expression::Construct(construct) => {
                self.scan_construct(construct, deps);
            }
            Expression::Pipe(pipe) => {
                self.scan_expression(pipe.input, deps);
                self.scan_expression(pipe.callable, deps);
            }
            Expression::AnonymousClass(anonymous_class) => {
                if let Some(extends) = &anonymous_class.extends {
                    for parent in extends.types.iter() {
                        deps.insert(self.normalize_name(parent.value()));
                    }
                }
                if let Some(implements) = &anonymous_class.implements {
                    for iface in implements.types.iter() {
                        deps.insert(self.normalize_name(iface.value()));
                    }
                }
                if let Some(argument_list) = &anonymous_class.argument_list {
                    for argument in argument_list.arguments.iter() {
                        self.scan_argument(argument, deps);
                    }
                }
            }
            Expression::PartialApplication(partial) => {
                self.scan_partial_application(partial, deps);
            }
            _ => {}
        }
    }

    fn scan_class_expression(&self, expression: &Expression<'_>, deps: &mut HashSet<String>) {
        match expression {
            Expression::Identifier(identifier) => {
                let name = self.normalize_name(identifier.value());
                if !self.is_builtin(&name) {
                    deps.insert(name);
                }
            }
            Expression::Parenthesized(parenthesized) => {
                self.scan_class_expression(parenthesized.expression, deps);
            }
            _ => {
                self.scan_expression(expression, deps);
            }
        }
    }

    fn scan_argument(&self, argument: &Argument<'_>, deps: &mut HashSet<String>) {
        match argument {
            Argument::Positional(pos) => self.scan_expression(pos.value, deps),
            Argument::Named(named) => self.scan_expression(named.value, deps),
        }
    }

    fn scan_array_element(&self, element: &ArrayElement<'_>, deps: &mut HashSet<String>) {
        if let Some(key) = element.get_key() {
            self.scan_expression(key, deps);
        }
        if let Some(value) = element.get_value() {
            self.scan_expression(value, deps);
        }
    }

    fn scan_construct(&self, construct: &Construct<'_>, deps: &mut HashSet<String>) {
        match construct {
            Construct::Isset(construct) => {
                for value in construct.values.iter() { self.scan_expression(value, deps); }
            }
            Construct::Empty(construct) => self.scan_expression(construct.value, deps),
            Construct::Eval(construct) => self.scan_expression(construct.value, deps),
            Construct::Include(construct) => self.scan_expression(construct.value, deps),
            Construct::IncludeOnce(construct) => self.scan_expression(construct.value, deps),
            Construct::Require(construct) => self.scan_expression(construct.value, deps),
            Construct::RequireOnce(construct) => self.scan_expression(construct.value, deps),
            Construct::Print(construct) => self.scan_expression(construct.value, deps),
            Construct::Exit(construct) => {
                if let Some(arguments) = &construct.arguments {
                    for argument in arguments.arguments.iter() {
                        self.scan_argument(argument, deps);
                    }
                }
            }
            Construct::Die(construct) => {
                if let Some(arguments) = &construct.arguments {
                    for argument in arguments.arguments.iter() {
                        self.scan_argument(argument, deps);
                    }
                }
            }
        }
    }

    fn scan_partial_application(&self, partial: &PartialApplication<'_>, deps: &mut HashSet<String>) {
        match partial {
            PartialApplication::Function(partial) => {
                self.scan_partial_arguments(&partial.argument_list, deps);
            }
            PartialApplication::Method(partial) => {
                self.scan_expression(partial.object, deps);
                self.scan_partial_arguments(&partial.argument_list, deps);
            }
            PartialApplication::StaticMethod(partial) => {
                self.scan_class_expression(partial.class, deps);
                self.scan_partial_arguments(&partial.argument_list, deps);
            }
        }
    }

    fn scan_partial_arguments(&self, argument_list: &PartialArgumentList<'_>, deps: &mut HashSet<String>) {
        for argument in argument_list.arguments.iter() {
            match argument {
                PartialArgument::Positional(arg) => self.scan_expression(arg.value, deps),
                PartialArgument::Named(arg) => self.scan_expression(arg.value, deps),
                _ => {}
            }
        }
    }

    fn compute_fan_in_out(
        &self,
        class_name: &str,
        index: &ClassIndex,
    ) -> (usize, usize) {
        let fan_out = index
            .class_dependencies
            .get(class_name)
            .map(|deps| deps.len())
            .unwrap_or(0);

        let fan_in = index
            .class_dependencies
            .iter()
            .filter(|(name, deps)| {
                *name != class_name && deps.contains(class_name)
            })
            .count();

        (fan_out, fan_in)
    }

    fn normalize_name(&self, name: &str) -> String {
        let normalized = name.trim_start_matches('\\');
        normalized
            .rsplit('\\')
            .next()
            .unwrap_or(normalized)
            .to_string()
    }

    fn is_builtin(&self, name: &str) -> bool {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "array" | "bool" | "boolean" | "callable" | "false" | "float"
            | "int" | "integer" | "iterable" | "mixed" | "never" | "null"
            | "object" | "resource" | "string" | "true" | "void"
            | "self" | "static" | "parent"
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn high_fan_out() {
        let violations = analyze_file_for_rule("e29/high_fan_out.php", CODE);
        assert!(violations.len().gt(&0));
        assert!(violations[0].suggestion.contains("fan-out"));
    }

    #[test]
    fn low_coupling() {
        let violations = analyze_file_for_rule("e29/low_coupling.php", CODE);
        assert!(violations.len().eq(&0));
    }
}
