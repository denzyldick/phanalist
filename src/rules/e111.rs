use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::{function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0111";
static DESCRIPTION: &str = "Deprecated mysql extension";

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
            walk_statement_expressions(statement, &mut |expression| {
                let Expression::Call(Call::Function(call)) = expression else {
                    return;
                };
                let Some(name) = function_name(call.function) else {
                    return;
                };
                if !is_mysql_extension(&name) {
                    return;
                }

                let call = Call::Function(call.clone());
                let message = Message::new(
                    format!("E0111:deprecated-mysql:{name}"),
                    "{}() is part of the removed ext/mysql extension. Use MySQLi or PDO instead.",
                )
                .arg("name", name);

                violations.push(self.new_violation(file, message, call.span()));
            });
        }

        Some(violations)
    }
}

fn is_mysql_extension(name: &str) -> bool {
    matches!(
        name,
        "mysql_connect" | "mysql_query" | "mysql_fetch_array" | "mysql_close" | "mysql_select_db"
    )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_deprecated_mysql() {
        let violations = analyze_file_for_rule("e111/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e111/safe.php", CODE).is_empty());
    }
}
