use std::collections::BTreeSet;

use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_arguments, direct_variable_name, function_name, method_name, walk_own_expressions,
};
use crate::rules::Rule as _;

pub(crate) static CODE: &str = "E0048";
static DESCRIPTION: &str = "Unbounded direct recursion";

#[derive(Default)]
pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        false
    }

    fn validate(&self, _file: &File<'_>, _statement: &Statement<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn validate_file(&self, file: &File<'_>) -> Option<Vec<Violation>> {
        let program = file.ast?;
        let mut violations = Vec::new();

        for statement in program.statements.iter() {
            self.check_statement(file, statement, &mut violations);
        }

        Some(violations)
    }
}

impl Rule {
    fn check_statement(
        &self,
        file: &File<'_>,
        statement: &Statement<'_>,
        violations: &mut Vec<Violation>,
    ) {
        match statement {
            Statement::Function(function) => {
                let name = String::from_utf8_lossy(function.name.value).to_string();
                self.check_body(
                    file,
                    &name,
                    None,
                    &function.parameter_list,
                    &function.body,
                    violations,
                );
            }
            Statement::Class(class) => {
                let class_name = String::from_utf8_lossy(class.name.value).to_string();
                self.check_members(file, &class_name, class.members.as_slice(), violations);
            }
            Statement::Interface(interface) => {
                let class_name = String::from_utf8_lossy(interface.name.value).to_string();
                self.check_members(file, &class_name, interface.members.as_slice(), violations);
            }
            Statement::Trait(trait_) => {
                let class_name = String::from_utf8_lossy(trait_.name.value).to_string();
                self.check_members(file, &class_name, trait_.members.as_slice(), violations);
            }
            Statement::Enum(enum_) => {
                let class_name = String::from_utf8_lossy(enum_.name.value).to_string();
                self.check_members(file, &class_name, enum_.members.as_slice(), violations);
            }
            Statement::Namespace(namespace) => {
                for statement in namespace.statements().iter() {
                    self.check_statement(file, statement, violations);
                }
            }
            _ => {}
        }
    }

    fn check_members(
        &self,
        file: &File<'_>,
        class_name: &str,
        members: &[ClassLikeMember<'_>],
        violations: &mut Vec<Violation>,
    ) {
        for member in members {
            if let ClassLikeMember::Method(method) = member {
                if let MethodBody::Concrete(block) = &method.body {
                    let name = String::from_utf8_lossy(method.name.value).to_string();
                    self.check_body(
                        file,
                        &name,
                        Some(class_name),
                        &method.parameter_list,
                        block,
                        violations,
                    );
                }
            }
        }
    }

    fn statements_with_switch<'a>(
        &'a self,
        statement: &'a Statement<'a>,
    ) -> Vec<&'a Statement<'a>> {
        let mut statements = Vec::new();
        self.collect_statements_with_switch(statement, &mut statements);
        statements
    }

    fn collect_statements_with_switch<'a>(
        &'a self,
        statement: &'a Statement<'a>,
        statements: &mut Vec<&'a Statement<'a>>,
    ) {
        for nested in self.flatten_statements_to_validate(statement) {
            statements.push(nested);

            if let Statement::Switch(switch) = nested {
                for case in switch.body.cases().iter() {
                    for statement in case.statements().iter() {
                        self.collect_statements_with_switch(statement, statements);
                    }
                }
            }
        }
    }

    fn check_body(
        &self,
        file: &File<'_>,
        name: &str,
        class_name: Option<&str>,
        parameter_list: &FunctionLikeParameterList<'_>,
        body: &Block<'_>,
        violations: &mut Vec<Violation>,
    ) {
        let parameters = parameter_list
            .parameters
            .iter()
            .map(|parameter| String::from_utf8_lossy(parameter.variable.name).to_string())
            .collect::<Vec<_>>();

        if parameter_list
            .parameters
            .iter()
            .any(|parameter| parameter.is_variadic())
        {
            return;
        }

        let mut mutated = BTreeSet::new();
        let mut shadowed = BTreeSet::new();
        let mut state_mutated = false;

        for statement in body.statements.iter() {
            for nested in self.statements_with_switch(statement) {
                match nested {
                    Statement::Foreach(foreach) => {
                        mark_parameter(foreach.target.value(), &parameters, &mut mutated);
                        if let Some(key) = foreach.target.key() {
                            mark_parameter(key, &parameters, &mut mutated);
                        }
                    }
                    Statement::Unset(unset) => {
                        for value in unset.values.iter() {
                            mark_parameter(value, &parameters, &mut mutated);
                        }
                    }
                    _ => {}
                }

                walk_own_expressions(nested, &mut |expression| {
                    if let Expression::Closure(closure) = expression {
                        for parameter in closure.parameter_list.parameters.iter() {
                            shadowed.insert(
                                String::from_utf8_lossy(parameter.variable.name).to_string(),
                            );
                        }
                    }
                    if let Expression::ArrowFunction(function) = expression {
                        for parameter in function.parameter_list.parameters.iter() {
                            shadowed.insert(
                                String::from_utf8_lossy(parameter.variable.name).to_string(),
                            );
                        }
                    }

                    let target = match expression {
                        Expression::Assignment(assignment) => Some(assignment.lhs),
                        Expression::UnaryPrefix(unary)
                            if matches!(
                                unary.operator,
                                UnaryPrefixOperator::PreIncrement(_)
                                    | UnaryPrefixOperator::PreDecrement(_)
                            ) =>
                        {
                            Some(unary.operand)
                        }
                        Expression::UnaryPostfix(unary)
                            if matches!(
                                unary.operator,
                                UnaryPostfixOperator::PostIncrement(_)
                                    | UnaryPostfixOperator::PostDecrement(_)
                            ) =>
                        {
                            Some(unary.operand)
                        }
                        _ => None,
                    };

                    if let Some(target) = target {
                        if !mark_parameter(target, &parameters, &mut mutated)
                            && is_state_access(target)
                        {
                            state_mutated = true;
                        }
                    }

                    let Expression::Call(call) = expression else {
                        return;
                    };
                    if let Call::Method(method_call) = call {
                        if !is_recursive_call(call, name, class_name)
                            && root_variable(method_call.object)
                                .is_some_and(|variable| variable.trim_start_matches('$') == "this")
                        {
                            state_mutated = true;
                        }
                    }
                    let Some(function) = call_function_name(call) else {
                        return;
                    };
                    if !MUTATING_BUILTINS.contains(&function.as_str()) {
                        return;
                    }
                    for argument in call_arguments(call) {
                        mark_parameter(argument, &parameters, &mut mutated);
                    }
                });
            }
        }

        if !mutated.is_empty() || state_mutated {
            return;
        }

        for statement in body.statements.iter() {
            for nested in self.statements_with_switch(statement) {
                walk_own_expressions(nested, &mut |expression| {
                    let Expression::Call(call) = expression else {
                        return;
                    };

                    if !is_recursive_call(call, name, class_name) {
                        return;
                    }

                    if !arguments_are_unchanged(call, &parameters, &shadowed) {
                        return;
                    }

                    let message = Message::new(
                        "E0048:unbounded-recursion",
                        "The call to {name}() is directly recursive with unchanged arguments and no parameter reassignment. Add a base case that changes the recursion argument, or document why the call terminates.",
                    )
                    .arg("name", name);

                    violations.push(self.new_violation(file, message, call.span()));
                });
            }
        }
    }
}

fn is_recursive_call(call: &Call<'_>, name: &str, class_name: Option<&str>) -> bool {
    match call {
        Call::Function(call) => {
            class_name.is_none()
                && !is_global_function_call(call.function)
                && function_name(call.function)
                    .is_some_and(|function| function.eq_ignore_ascii_case(name))
        }
        Call::Method(call) => {
            class_name.is_some()
                && method_name(&call.method).is_some_and(|method| method.eq_ignore_ascii_case(name))
                && direct_variable_name(call.object)
                    .is_some_and(|variable| variable.trim_start_matches('$') == "this")
        }
        Call::NullSafeMethod(_) => false,
        Call::StaticMethod(call) => {
            class_name.is_some()
                && method_name(&call.method).is_some_and(|method| method.eq_ignore_ascii_case(name))
                && is_same_class_expression(call.class, class_name.unwrap_or_default())
        }
    }
}

fn is_global_function_call(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Identifier(identifier) => identifier.value().starts_with(b"\\"),
        Expression::Parenthesized(parenthesized) => {
            is_global_function_call(parenthesized.expression)
        }
        _ => false,
    }
}

fn call_function_name(call: &Call<'_>) -> Option<String> {
    let Call::Function(call) = call else {
        return None;
    };
    function_name(call.function)
}

fn root_variable<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::Variable(Variable::Direct(variable)) => std::str::from_utf8(variable.name).ok(),
        Expression::Parenthesized(parenthesized) => root_variable(parenthesized.expression),
        Expression::ArrayAccess(access) => root_variable(access.array),
        Expression::ArrayAppend(append) => root_variable(append.array),
        Expression::Access(Access::Property(access)) => root_variable(access.object),
        Expression::Access(Access::NullSafeProperty(access)) => root_variable(access.object),
        _ => None,
    }
}

fn mark_parameter(
    expression: &Expression<'_>,
    parameters: &[String],
    mutated: &mut BTreeSet<String>,
) -> bool {
    let Some(variable) = root_variable(expression) else {
        return false;
    };
    if !parameters.iter().any(|parameter| parameter == variable) {
        return false;
    }
    mutated.insert(variable.to_string());
    true
}

fn is_state_access(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Access(Access::Property(access)) => direct_variable_name(access.object)
            .is_some_and(|variable| variable.trim_start_matches('$') == "this"),
        Expression::Access(Access::NullSafeProperty(access)) => direct_variable_name(access.object)
            .is_some_and(|variable| variable.trim_start_matches('$') == "this"),
        Expression::Access(Access::StaticProperty(_)) => true,
        _ => false,
    }
}

fn is_same_class_expression(expression: &Expression<'_>, class_name: &str) -> bool {
    let name = match expression {
        Expression::Identifier(identifier) => {
            String::from_utf8_lossy(identifier.value()).to_string()
        }
        Expression::Self_(_) => String::from("self"),
        Expression::Static(_) => String::from("static"),
        _ => return false,
    };

    name.eq_ignore_ascii_case("self")
        || name.eq_ignore_ascii_case("static")
        || name.eq_ignore_ascii_case(class_name)
        || name.eq_ignore_ascii_case(&format!("\\{class_name}"))
}

const MUTATING_BUILTINS: &[&str] = &[
    "array_pop",
    "array_push",
    "array_shift",
    "array_unshift",
    "array_splice",
    "array_multisort",
    "array_walk",
    "array_walk_recursive",
    "sort",
    "rsort",
    "usort",
    "uasort",
    "uksort",
    "shuffle",
    "settype",
    "parse_str",
    "preg_match",
    "preg_match_all",
];

fn arguments_are_unchanged(
    call: &Call<'_>,
    parameters: &[String],
    shadowed: &BTreeSet<String>,
) -> bool {
    let arguments = call_arguments(call);
    if arguments.len() != parameters.len() {
        return false;
    }

    if arguments.is_empty() && !shadowed.is_empty() {
        return false;
    }

    arguments
        .iter()
        .zip(parameters)
        .all(|(argument, parameter)| {
            direct_variable_name(argument)
                .is_some_and(|variable| variable == parameter && !shadowed.contains(variable))
        })
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_direct_recursion_with_unchanged_arguments() {
        assert!(analyze_file_for_rule("e48/function_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e48/method_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e48/zero_arg_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e48/switch_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_changing_or_reassigned_arguments() {
        assert!(analyze_file_for_rule("e48/function_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/method_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/reassigned_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/mutated_array_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/property_state_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/object_parameter_state_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/this_method_state_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/method_function_collision_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/foreach_target_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/global_function_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/closure_shadow_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/state_guard_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/parent_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e48/switch_foreach_safe.php", CODE).is_empty());
    }
}
