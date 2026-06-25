use std::collections::HashSet;

use mago_span::HasSpan;
use mago_syntax::cst::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0017";
static DESCRIPTION: &str = "Coupling Between Objects (CBO)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_coupling: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_coupling: 10 }
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
            let current_class = String::from_utf8_lossy(class.name.value).into_owned();
            let mut coupled_types = HashSet::new();

            if let Some(extends) = &class.extends {
                for parent in extends.types.iter() {
                    self.add_type_name(std::str::from_utf8(parent.value()).unwrap_or_default(), &current_class, &mut coupled_types);
                }
            }

            if let Some(implements) = &class.implements {
                for interface in implements.types.iter() {
                    self.add_type_name(std::str::from_utf8(interface.value()).unwrap_or_default(), &current_class, &mut coupled_types);
                }
            }

            for member in class.members.iter() {
                self.scan_class_member(member, &current_class, &mut coupled_types);
            }

            let coupling = coupled_types.len();
            if coupling > self.settings.max_coupling {
                let mut names = coupled_types.into_iter().collect::<Vec<_>>();
                names.sort();
                let message = Message::new(
                    "E0017:high-coupling",
                    "Class \"{class}\" is coupled to {coupling} external types ({types}). Reduce the number of collaborators or split responsibilities.",
                )
                .arg("class", current_class)
                .arg("coupling", coupling.to_string())
                .arg("types", names.join(", "));
                violations.push(self.new_violation(file, message, class.span()));
            }
        }

        violations
    }
}

impl Rule {
    fn scan_class_member(
        &self,
        member: &ClassLikeMember<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        match member {
            ClassLikeMember::Method(method) => {
                self.scan_parameters(&method.parameter_list, current_class, coupled_types);
                if let Some(return_hint) = &method.return_type_hint {
                    self.scan_hint(&return_hint.hint, current_class, coupled_types);
                }
                if let MethodBody::Concrete(block) = &method.body {
                    for statement in block.statements.iter() {
                        self.scan_statement(statement, current_class, coupled_types);
                    }
                }
            }
            ClassLikeMember::Property(property) => {
                if let Some(hint) = property.hint() {
                    self.scan_hint(hint, current_class, coupled_types);
                }
                if let Property::Plain(plain) = property {
                    for item in plain.items.iter() {
                        if let PropertyItem::Concrete(item) = item {
                            self.scan_expression(item.value, current_class, coupled_types);
                        }
                    }
                }

                if let Property::Hooked(hooked) = property {
                    for hook in hooked.hook_list.hooks.iter() {
                        if let Some(parameters) = &hook.parameter_list {
                            self.scan_parameters(parameters, current_class, coupled_types);
                        }
                        match &hook.body {
                            PropertyHookBody::Concrete(PropertyHookConcreteBody::Block(block)) => {
                                for statement in block.statements.iter() {
                                    self.scan_statement(statement, current_class, coupled_types);
                                }
                            }
                            PropertyHookBody::Concrete(PropertyHookConcreteBody::Expression(
                                expr,
                            )) => {
                                self.scan_expression(expr.expression, current_class, coupled_types);
                            }
                            _ => {}
                        }
                    }
                }
            }
            ClassLikeMember::Constant(constant) => {
                if let Some(hint) = &constant.hint {
                    self.scan_hint(hint, current_class, coupled_types);
                }
                for item in constant.items.iter() {
                    self.scan_expression(item.value, current_class, coupled_types);
                }
            }
            ClassLikeMember::TraitUse(trait_use) => {
                for trait_name in trait_use.trait_names.iter() {
                    self.add_type_name(std::str::from_utf8(trait_name.value()).unwrap_or_default(), current_class, coupled_types);
                }
            }
            _ => {}
        }
    }

    fn scan_parameters(
        &self,
        parameters: &FunctionLikeParameterList<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        for parameter in parameters.parameters.iter() {
            if let Some(hint) = &parameter.hint {
                self.scan_hint(hint, current_class, coupled_types);
            }
            if let Some(default) = &parameter.default_value {
                self.scan_expression(default.value, current_class, coupled_types);
            }
        }
    }

    fn scan_hint(&self, hint: &Hint<'_>, current_class: &str, coupled_types: &mut HashSet<String>) {
        match hint {
            Hint::Identifier(identifier) => {
                self.add_type_name(std::str::from_utf8(identifier.value()).unwrap_or_default(), current_class, coupled_types);
            }
            Hint::Parenthesized(parenthesized) => {
                self.scan_hint(parenthesized.hint, current_class, coupled_types);
            }
            Hint::Nullable(nullable) => {
                self.scan_hint(nullable.hint, current_class, coupled_types);
            }
            Hint::Union(union) => {
                self.scan_hint(union.left, current_class, coupled_types);
                self.scan_hint(union.right, current_class, coupled_types);
            }
            Hint::Intersection(intersection) => {
                self.scan_hint(intersection.left, current_class, coupled_types);
                self.scan_hint(intersection.right, current_class, coupled_types);
            }
            _ => {}
        }
    }

    fn scan_statement(
        &self,
        statement: &Statement<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        match statement {
            Statement::Expression(expr) => {
                self.scan_expression(expr.expression, current_class, coupled_types);
            }
            Statement::Return(ret) => {
                if let Some(value) = ret.value {
                    self.scan_expression(value, current_class, coupled_types);
                }
            }
            Statement::Echo(echo) => {
                for value in echo.values.iter() {
                    self.scan_expression(value, current_class, coupled_types);
                }
            }
            Statement::Block(block) => {
                for statement in block.statements.iter() {
                    self.scan_statement(statement, current_class, coupled_types);
                }
            }
            Statement::If(if_stmt) => {
                self.scan_expression(if_stmt.condition, current_class, coupled_types);
                match &if_stmt.body {
                    IfBody::Statement(body) => {
                        self.scan_statement(body.statement, current_class, coupled_types);
                        for clause in body.else_if_clauses.iter() {
                            self.scan_expression(clause.condition, current_class, coupled_types);
                            self.scan_statement(clause.statement, current_class, coupled_types);
                        }
                        if let Some(else_clause) = &body.else_clause {
                            self.scan_statement(
                                else_clause.statement,
                                current_class,
                                coupled_types,
                            );
                        }
                    }
                    IfBody::ColonDelimited(body) => {
                        for statement in body.statements.iter() {
                            self.scan_statement(statement, current_class, coupled_types);
                        }
                        for clause in body.else_if_clauses.iter() {
                            self.scan_expression(clause.condition, current_class, coupled_types);
                            for statement in clause.statements.iter() {
                                self.scan_statement(statement, current_class, coupled_types);
                            }
                        }
                        if let Some(else_clause) = &body.else_clause {
                            for statement in else_clause.statements.iter() {
                                self.scan_statement(statement, current_class, coupled_types);
                            }
                        }
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.scan_expression(while_stmt.condition, current_class, coupled_types);
                match &while_stmt.body {
                    WhileBody::Statement(body) => {
                        self.scan_statement(body, current_class, coupled_types);
                    }
                    WhileBody::ColonDelimited(body) => {
                        for statement in body.statements.iter() {
                            self.scan_statement(statement, current_class, coupled_types);
                        }
                    }
                }
            }
            Statement::DoWhile(do_while) => {
                self.scan_statement(do_while.statement, current_class, coupled_types);
                self.scan_expression(do_while.condition, current_class, coupled_types);
            }
            Statement::For(for_stmt) => {
                for init in for_stmt.initializations.iter() {
                    self.scan_expression(init, current_class, coupled_types);
                }
                for condition in for_stmt.conditions.iter() {
                    self.scan_expression(condition, current_class, coupled_types);
                }
                for loop_expr in for_stmt.increments.iter() {
                    self.scan_expression(loop_expr, current_class, coupled_types);
                }
                match &for_stmt.body {
                    ForBody::Statement(body) => {
                        self.scan_statement(body, current_class, coupled_types);
                    }
                    ForBody::ColonDelimited(body) => {
                        for statement in body.statements.iter() {
                            self.scan_statement(statement, current_class, coupled_types);
                        }
                    }
                }
            }
            Statement::Foreach(foreach) => {
                self.scan_expression(foreach.expression, current_class, coupled_types);
                match &foreach.target {
                    ForeachTarget::Value(target) => {
                        self.scan_expression(target.value, current_class, coupled_types);
                    }
                    ForeachTarget::KeyValue(target) => {
                        self.scan_expression(target.key, current_class, coupled_types);
                        self.scan_expression(target.value, current_class, coupled_types);
                    }
                }
                match &foreach.body {
                    ForeachBody::Statement(body) => {
                        self.scan_statement(body, current_class, coupled_types);
                    }
                    ForeachBody::ColonDelimited(body) => {
                        for statement in body.statements.iter() {
                            self.scan_statement(statement, current_class, coupled_types);
                        }
                    }
                }
            }
            Statement::Switch(switch) => {
                self.scan_expression(switch.expression, current_class, coupled_types);
                let cases = match &switch.body {
                    SwitchBody::BraceDelimited(body) => &body.cases,
                    SwitchBody::ColonDelimited(body) => &body.cases,
                };
                for case in cases.iter() {
                    match case {
                        SwitchCase::Expression(case) => {
                            self.scan_expression(case.expression, current_class, coupled_types);
                            for statement in case.statements.iter() {
                                self.scan_statement(statement, current_class, coupled_types);
                            }
                        }
                        SwitchCase::Default(case) => {
                            for statement in case.statements.iter() {
                                self.scan_statement(statement, current_class, coupled_types);
                            }
                        }
                    }
                }
            }
            Statement::Try(try_stmt) => {
                for statement in try_stmt.block.statements.iter() {
                    self.scan_statement(statement, current_class, coupled_types);
                }
                for catch in try_stmt.catch_clauses.iter() {
                    self.scan_hint(&catch.hint, current_class, coupled_types);
                    for statement in catch.block.statements.iter() {
                        self.scan_statement(statement, current_class, coupled_types);
                    }
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    for statement in finally.block.statements.iter() {
                        self.scan_statement(statement, current_class, coupled_types);
                    }
                }
            }
            _ => {}
        }
    }

    fn scan_expression(
        &self,
        expression: &Expression<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        match expression {
            Expression::Instantiation(instantiation) => {
                self.scan_class_expression(instantiation.class, current_class, coupled_types);
                if let Some(arguments) = &instantiation.argument_list {
                    for argument in arguments.arguments.iter() {
                        self.scan_argument(argument, current_class, coupled_types);
                    }
                }
            }
            Expression::Call(call) => match call {
                Call::Method(method) => {
                    self.scan_expression(method.object, current_class, coupled_types);
                    for argument in method.argument_list.arguments.iter() {
                        self.scan_argument(argument, current_class, coupled_types);
                    }
                }
                Call::NullSafeMethod(method) => {
                    self.scan_expression(method.object, current_class, coupled_types);
                    for argument in method.argument_list.arguments.iter() {
                        self.scan_argument(argument, current_class, coupled_types);
                    }
                }
                Call::StaticMethod(method) => {
                    self.scan_class_expression(method.class, current_class, coupled_types);
                    for argument in method.argument_list.arguments.iter() {
                        self.scan_argument(argument, current_class, coupled_types);
                    }
                }
                Call::Function(function) => {
                    for argument in function.argument_list.arguments.iter() {
                        self.scan_argument(argument, current_class, coupled_types);
                    }
                }
            },
            Expression::Access(access) => match access {
                Access::Property(property) => {
                    self.scan_expression(property.object, current_class, coupled_types);
                }
                Access::NullSafeProperty(property) => {
                    self.scan_expression(property.object, current_class, coupled_types);
                }
                Access::ClassConstant(constant) => {
                    self.scan_class_expression(constant.class, current_class, coupled_types);
                }
                Access::StaticProperty(property) => {
                    self.scan_class_expression(property.class, current_class, coupled_types);
                }
            },
            Expression::Binary(binary) => {
                self.scan_expression(binary.lhs, current_class, coupled_types);
                if matches!(binary.operator, BinaryOperator::Instanceof(_)) {
                    self.scan_class_expression(binary.rhs, current_class, coupled_types);
                } else {
                    self.scan_expression(binary.rhs, current_class, coupled_types);
                }
            }
            Expression::UnaryPrefix(unary) => {
                self.scan_expression(unary.operand, current_class, coupled_types);
            }
            Expression::UnaryPostfix(unary) => {
                self.scan_expression(unary.operand, current_class, coupled_types);
            }
            Expression::Assignment(assignment) => {
                self.scan_expression(assignment.lhs, current_class, coupled_types);
                self.scan_expression(assignment.rhs, current_class, coupled_types);
            }
            Expression::Parenthesized(parenthesized) => {
                self.scan_expression(parenthesized.expression, current_class, coupled_types);
            }
            Expression::Array(array) => {
                for element in array.elements.iter() {
                    self.scan_array_element(element, current_class, coupled_types);
                }
            }
            Expression::LegacyArray(array) => {
                for element in array.elements.iter() {
                    self.scan_array_element(element, current_class, coupled_types);
                }
            }
            Expression::List(list) => {
                for element in list.elements.iter() {
                    self.scan_array_element(element, current_class, coupled_types);
                }
            }
            Expression::ArrayAccess(array_access) => {
                self.scan_expression(array_access.array, current_class, coupled_types);
                self.scan_expression(array_access.index, current_class, coupled_types);
            }
            Expression::ArrayAppend(array_append) => {
                self.scan_expression(array_append.array, current_class, coupled_types);
            }
            Expression::Closure(closure) => {
                self.scan_parameters(&closure.parameter_list, current_class, coupled_types);
                if let Some(return_hint) = &closure.return_type_hint {
                    self.scan_hint(&return_hint.hint, current_class, coupled_types);
                }
                for statement in closure.body.statements.iter() {
                    self.scan_statement(statement, current_class, coupled_types);
                }
            }
            Expression::ArrowFunction(arrow) => {
                self.scan_parameters(&arrow.parameter_list, current_class, coupled_types);
                if let Some(return_hint) = &arrow.return_type_hint {
                    self.scan_hint(&return_hint.hint, current_class, coupled_types);
                }
                self.scan_expression(arrow.expression, current_class, coupled_types);
            }
            Expression::Conditional(conditional) => {
                self.scan_expression(conditional.condition, current_class, coupled_types);
                if let Some(then_expr) = conditional.then {
                    self.scan_expression(then_expr, current_class, coupled_types);
                }
                self.scan_expression(conditional.r#else, current_class, coupled_types);
            }
            Expression::Match(match_expr) => {
                self.scan_expression(match_expr.expression, current_class, coupled_types);
                for arm in match_expr.arms.iter() {
                    match arm {
                        MatchArm::Expression(arm) => {
                            for condition in arm.conditions.iter() {
                                self.scan_expression(condition, current_class, coupled_types);
                            }
                            self.scan_expression(arm.expression, current_class, coupled_types);
                        }
                        MatchArm::Default(arm) => {
                            self.scan_expression(arm.expression, current_class, coupled_types);
                        }
                    }
                }
            }
            Expression::Throw(throw) => {
                self.scan_expression(throw.exception, current_class, coupled_types);
            }
            Expression::Clone(clone) => {
                self.scan_expression(clone.object, current_class, coupled_types);
            }
            Expression::Yield(yield_expr) => match yield_expr {
                Yield::Value(value) => {
                    if let Some(value) = value.value {
                        self.scan_expression(value, current_class, coupled_types);
                    }
                }
                Yield::Pair(pair) => {
                    self.scan_expression(pair.key, current_class, coupled_types);
                    self.scan_expression(pair.value, current_class, coupled_types);
                }
                Yield::From(from) => {
                    self.scan_expression(from.iterator, current_class, coupled_types);
                }
            },
            Expression::Construct(construct) => {
                self.scan_construct(construct, current_class, coupled_types);
            }
            Expression::PartialApplication(partial) => {
                self.scan_partial_application(partial, current_class, coupled_types);
            }
            Expression::Pipe(pipe) => {
                self.scan_expression(pipe.input, current_class, coupled_types);
                self.scan_expression(pipe.callable, current_class, coupled_types);
            }
            Expression::AnonymousClass(anonymous_class) => {
                if let Some(extends) = &anonymous_class.extends {
                    for parent in extends.types.iter() {
                        self.add_type_name(std::str::from_utf8(parent.value()).unwrap_or_default(), current_class, coupled_types);
                    }
                }
                if let Some(implements) = &anonymous_class.implements {
                    for interface in implements.types.iter() {
                        self.add_type_name(std::str::from_utf8(interface.value()).unwrap_or_default(), current_class, coupled_types);
                    }
                }
                if let Some(argument_list) = &anonymous_class.argument_list {
                    for argument in argument_list.arguments.iter() {
                        if let Some(value) = argument.value() {
                            self.scan_expression(value, current_class, coupled_types);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn scan_class_expression(
        &self,
        expression: &Expression<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        match expression {
            Expression::Identifier(identifier) => {
                self.add_type_name(std::str::from_utf8(identifier.value()).unwrap_or_default(), current_class, coupled_types);
            }
            Expression::Parenthesized(parenthesized) => {
                self.scan_class_expression(parenthesized.expression, current_class, coupled_types);
            }
            _ => {
                self.scan_expression(expression, current_class, coupled_types);
            }
        }
    }

    fn scan_array_element(
        &self,
        element: &ArrayElement<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        if let Some(key) = element.get_key() {
            self.scan_expression(key, current_class, coupled_types);
        }
        if let Some(value) = element.get_value() {
            self.scan_expression(value, current_class, coupled_types);
        }
    }

    fn scan_construct(
        &self,
        construct: &Construct<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        match construct {
            Construct::Isset(construct) => {
                for value in construct.values.iter() {
                    self.scan_expression(value, current_class, coupled_types);
                }
            }
            Construct::Empty(construct) => {
                self.scan_expression(construct.value, current_class, coupled_types);
            }
            Construct::Eval(construct) => {
                self.scan_expression(construct.value, current_class, coupled_types);
            }
            Construct::Include(construct) => {
                self.scan_expression(construct.value, current_class, coupled_types);
            }
            Construct::IncludeOnce(construct) => {
                self.scan_expression(construct.value, current_class, coupled_types);
            }
            Construct::Require(construct) => {
                self.scan_expression(construct.value, current_class, coupled_types);
            }
            Construct::RequireOnce(construct) => {
                self.scan_expression(construct.value, current_class, coupled_types);
            }
            Construct::Print(construct) => {
                self.scan_expression(construct.value, current_class, coupled_types);
            }
            Construct::Exit(construct) => {
                if let Some(arguments) = &construct.arguments {
                    for argument in arguments.arguments.iter() {
                        self.scan_argument(argument, current_class, coupled_types);
                    }
                }
            }
            Construct::Die(construct) => {
                if let Some(arguments) = &construct.arguments {
                    for argument in arguments.arguments.iter() {
                        self.scan_argument(argument, current_class, coupled_types);
                    }
                }
            }
        }
    }

    fn scan_partial_application(
        &self,
        partial: &PartialApplication<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        match partial {
            PartialApplication::Function(partial) => {
                self.scan_partial_arguments(&partial.argument_list, current_class, coupled_types);
            }
            PartialApplication::Method(partial) => {
                self.scan_expression(partial.object, current_class, coupled_types);
                self.scan_partial_arguments(&partial.argument_list, current_class, coupled_types);
            }
            PartialApplication::StaticMethod(partial) => {
                self.scan_class_expression(partial.class, current_class, coupled_types);
                self.scan_partial_arguments(&partial.argument_list, current_class, coupled_types);
            }
        }
    }

    fn scan_partial_arguments(
        &self,
        argument_list: &PartialArgumentList<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        for argument in argument_list.arguments.iter() {
            match argument {
                PartialArgument::Positional(argument) => {
                    self.scan_expression(argument.value, current_class, coupled_types);
                }
                PartialArgument::Named(argument) => {
                    self.scan_expression(argument.value, current_class, coupled_types);
                }
                _ => {}
            }
        }
    }

    fn scan_argument(
        &self,
        argument: &Argument<'_>,
        current_class: &str,
        coupled_types: &mut HashSet<String>,
    ) {
        match argument {
            Argument::Positional(positional) => {
                self.scan_expression(positional.value, current_class, coupled_types);
            }
            Argument::Named(named) => {
                self.scan_expression(named.value, current_class, coupled_types);
            }
        }
    }

    fn add_type_name(&self, name: &str, current_class: &str, coupled_types: &mut HashSet<String>) {
        let normalized = name.trim_start_matches('\\');
        let short_name = normalized.rsplit('\\').next().unwrap_or(normalized);

        if normalized.is_empty()
            || normalized.eq_ignore_ascii_case(current_class)
            || short_name.eq_ignore_ascii_case(current_class)
            || matches!(short_name, "self" | "static" | "parent")
            || is_builtin_type(short_name)
        {
            return;
        }

        coupled_types.insert(normalized.to_string());
    }
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "array"
            | "bool"
            | "boolean"
            | "callable"
            | "false"
            | "float"
            | "int"
            | "integer"
            | "iterable"
            | "mixed"
            | "never"
            | "null"
            | "object"
            | "resource"
            | "string"
            | "true"
            | "void"
    )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn high_coupling() {
        let violations = analyze_file_for_rule("e17/high_coupling.php", CODE);

        assert_eq!(violations.len(), 1);
        assert!(violations[0]
            .message
            .render()
            .contains("is coupled to 13 external types"));
        assert!(violations[0].message.render().contains("PaymentGateway"));
    }

    #[test]
    fn low_coupling() {
        let violations = analyze_file_for_rule("e17/low_coupling.php", CODE);

        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn function_calls_are_not_counted_as_type_coupling() {
        let violations = analyze_file_for_rule("e17/function_calls_not_types.php", CODE);

        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn self_builtins_and_duplicate_types_are_ignored() {
        let violations = analyze_file_for_rule("e17/self_and_duplicates.php", CODE);

        assert_eq!(violations.len(), 0);
    }
}
