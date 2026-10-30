use std::collections::{BTreeSet, HashSet};

use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_arguments, direct_variable_name, function_name, walk_expression,
    walk_statement_expressions,
};
use crate::rules::Rule as _;

pub(crate) static CODE: &str = "E0047";
static DESCRIPTION: &str = "Resource lifecycle leak";

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
            Statement::Function(function) => self.check_body(
                file,
                String::from_utf8_lossy(function.name.value).to_string(),
                &function.body,
                violations,
            ),
            Statement::Class(class) => {
                self.check_members(file, class.members.as_slice(), violations)
            }
            Statement::Interface(interface) => {
                self.check_members(file, interface.members.as_slice(), violations);
            }
            Statement::Trait(trait_) => {
                self.check_members(file, trait_.members.as_slice(), violations)
            }
            Statement::Enum(enum_) => {
                self.check_members(file, enum_.members.as_slice(), violations)
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
        members: &[ClassLikeMember<'_>],
        violations: &mut Vec<Violation>,
    ) {
        for member in members {
            if let ClassLikeMember::Method(method) = member {
                if let MethodBody::Concrete(block) = &method.body {
                    self.check_body(
                        file,
                        String::from_utf8_lossy(method.name.value).to_string(),
                        block,
                        violations,
                    );
                }
            }
        }
    }

    fn check_body(
        &self,
        file: &File<'_>,
        function: String,
        body: &Block<'_>,
        violations: &mut Vec<Violation>,
    ) {
        let mut scan = Scan::default();

        for statement in body.statements.iter() {
            self.escape_returned_resources(statement, &mut scan);
            self.scan_statement(statement, &mut scan);
        }

        for open in scan.opens {
            if !scan.used.contains(&open.variable)
                || scan.closed.contains(&open.variable)
                || scan.escaped.contains(&open.variable)
            {
                continue;
            }

            let message = Message::new(
                "E0047:resource-not-closed",
                "The resource returned by {function}() in {function_name}() is never closed. Call the matching close function on every path, or return/transfer ownership of the handle.",
            )
            .arg("function", open.function.clone())
            .arg("function_name", function.clone());

            violations.push(self.new_violation(file, message, open.span));
        }
    }

    fn escape_returned_resources(&self, statement: &Statement<'_>, scan: &mut Scan) {
        for nested in self.flatten_statements_to_validate(statement) {
            if let Statement::Return(return_) = nested {
                if let Some(value) = return_.value {
                    let mut variables = Vec::new();
                    walk_expression(value, &mut |expression| {
                        if let Some(variable) = direct_variable_name(expression) {
                            variables.push(variable.to_string());
                        }
                    });
                    for variable in variables {
                        scan.escaped.insert(variable);
                    }
                }
            }

            if let Statement::Switch(switch) = nested {
                for case in switch.body.cases().iter() {
                    for statement in case.statements().iter() {
                        self.escape_returned_resources(statement, scan);
                    }
                }
            }
        }
    }

    fn scan_statement(&self, statement: &Statement<'_>, scan: &mut Scan) {
        walk_statement_expressions(statement, &mut |expression| {
            self.scan_expression(expression, scan);
        });
    }

    fn scan_expression(&self, expression: &Expression<'_>, scan: &mut Scan) {
        match expression {
            Expression::Assignment(assignment) => {
                if let Some(variable) = direct_variable_name(assignment.lhs) {
                    if let Some(open) = open_function(assignment.rhs) {
                        scan.opens.push(OpenResource {
                            variable: variable.to_string(),
                            function: open.to_string(),
                            span: assignment.rhs.span(),
                        });
                    }
                } else {
                    let mut variables = Vec::new();
                    walk_expression(assignment.rhs, &mut |expression| {
                        if let Some(variable) = direct_variable_name(expression) {
                            variables.push(variable.to_string());
                        }
                    });
                    for variable in variables {
                        if scan.is_open(&variable) {
                            scan.escaped.insert(variable);
                        }
                    }
                }
            }
            Expression::Call(call) => self.scan_call(call, scan),
            Expression::Instantiation(instantiation) => {
                let Some(arguments) = &instantiation.argument_list else {
                    return;
                };
                for argument in arguments.arguments.iter() {
                    let value = match argument {
                        Argument::Positional(argument) => argument.value,
                        Argument::Named(argument) => argument.value,
                    };
                    if let Some(variable) = direct_variable_name(value) {
                        if scan.is_open(variable) {
                            scan.escaped.insert(variable.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn scan_call(&self, call: &Call<'_>, scan: &mut Scan) {
        let Some(name) = function_name_for_call(call) else {
            for argument in call_arguments(call) {
                if let Some(variable) = direct_variable_name(argument) {
                    if scan.is_open(variable) {
                        scan.escaped.insert(variable.to_string());
                    }
                }
            }
            return;
        };

        let arguments = call_arguments(call);

        if is_close_function(&name) {
            if let Some(variable) = arguments.first().and_then(|arg| direct_variable_name(arg)) {
                if scan.is_open(variable) {
                    scan.closed.insert(variable.to_string());
                }
            }
            return;
        }

        if is_resource_consumer(&name) {
            if let Some(variable) = arguments
                .first()
                .and_then(|argument| direct_variable_name(argument))
            {
                if scan.is_open(variable) {
                    scan.used.insert(variable.to_string());
                }
            }
            return;
        }

        if is_open_function(&name) {
            return;
        }

        for argument in arguments {
            if let Some(variable) = direct_variable_name(argument) {
                if scan.is_open(variable) {
                    scan.escaped.insert(variable.to_string());
                }
            }
        }
    }
}

#[derive(Default)]
struct Scan {
    opens: Vec<OpenResource>,
    used: BTreeSet<String>,
    closed: BTreeSet<String>,
    escaped: HashSet<String>,
}

impl Scan {
    fn is_open(&self, variable: &str) -> bool {
        self.opens
            .iter()
            .any(|open| open.variable == variable && !self.closed.contains(variable))
    }
}

struct OpenResource {
    variable: String,
    function: String,
    span: mago_span::Span,
}

fn function_name_for_call(call: &Call<'_>) -> Option<String> {
    let Call::Function(call) = call else {
        return None;
    };
    function_name(call.function)
}

fn open_function(expression: &Expression<'_>) -> Option<&'static str> {
    let Expression::Call(Call::Function(call)) = strip_parentheses(expression) else {
        return None;
    };
    let name = function_name(call.function)?;
    OPEN_FUNCTIONS
        .iter()
        .find(|function| **function == name)
        .copied()
}

fn is_open_function(name: &str) -> bool {
    OPEN_FUNCTIONS.contains(&name)
}

fn is_close_function(name: &str) -> bool {
    CLOSE_FUNCTIONS.contains(&name)
}

fn is_resource_consumer(name: &str) -> bool {
    RESOURCE_CONSUMERS.contains(&name)
}

fn strip_parentheses<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    match expression {
        Expression::Parenthesized(parenthesized) => strip_parentheses(parenthesized.expression),
        _ => expression,
    }
}

const OPEN_FUNCTIONS: &[&str] = &[
    "fopen",
    "fsockopen",
    "pfsockopen",
    "stream_socket_client",
    "tmpfile",
    "popen",
    "proc_open",
    "opendir",
];

const CLOSE_FUNCTIONS: &[&str] = &["fclose", "pclose", "proc_close", "closedir"];

const RESOURCE_CONSUMERS: &[&str] = &[
    "fread",
    "fwrite",
    "fputs",
    "fgets",
    "fgetc",
    "fgetcsv",
    "fputcsv",
    "fscanf",
    "fprintf",
    "vfprintf",
    "feof",
    "fflush",
    "fseek",
    "ftell",
    "rewind",
    "flock",
    "ftruncate",
    "fstat",
    "fpassthru",
    "stream_get_contents",
    "stream_set_blocking",
    "stream_set_timeout",
    "stream_get_meta_data",
    "proc_get_status",
    "proc_terminate",
    "readdir",
    "rewinddir",
];

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_unclosed_resources() {
        assert!(analyze_file_for_rule("e47/fopen_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e47/popen_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e47/opendir_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_closed_or_transferred_resources() {
        assert!(analyze_file_for_rule("e47/fopen_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e47/return_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e47/unused_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e47/switch_return_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e47/property_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e47/constructor_transfer_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e47/unknown_call_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e47/reassigned_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
    }
}
