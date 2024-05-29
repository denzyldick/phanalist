use php_parser_rs::parser::ast::{modifiers::MethodModifier::Static, Statement};

use crate::file::{self, File};
use crate::results::Violation;

static CODE: &str = "E0013";
static DESCRIPTION: &str = "Detect dead code.";

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let violations = Vec::new();

        violations
    }
    fn do_validate(&self, file: &File) -> bool {
        file.get_fully_qualified_name().is_some()
    }

    fn new_violation(
        &self,
        file: &File,
        suggestion: String,
        span: php_parser_rs::lexer::token::Span,
    ) -> Violation {
        let line = file.lines.get(span.line - 1).unwrap();

        Violation {
            rule: self.get_code(),
            line: String::from(line),
            suggestion,
            span,
        }
    }

    fn flatten_statements_to_validate<'a>(&'a self, statement: &'a Statement) -> Vec<&Statement> {
        let flatten_statements: Vec<&Statement> = Vec::new();

        self.travers_statements_to_validate(flatten_statements, statement)
    }

    fn travers_statements_to_validate<'a>(
        &'a self,
        mut flatten_statements: Vec<&'a Statement>,
        statement: &'a Statement,
    ) -> Vec<&Statement> {
        flatten_statements.push(statement);

        let child_statements: super::ast_child_statements::AstChildStatements = match statement {
            Statement::Namespace(statement) => statement.into(),
            Statement::Trait(statement) => statement.into(),
            Statement::Class(statement) => statement.into(),
            Statement::Block(statement) => statement.into(),
            Statement::If(statement) => statement.into(),
            Statement::Switch(statement) => statement.into(),
            Statement::While(statement) => statement.into(),
            Statement::Foreach(statement) => statement.into(),
            Statement::For(statement) => statement.into(),
            Statement::Try(statement) => statement.into(),
            _ => super::ast_child_statements::AstChildStatements { statements: vec![] },
        };

        for statement in child_statements.statements {
            flatten_statements.append(&mut self.flatten_statements_to_validate(statement));
        }

        flatten_statements
    }

    fn class_statements_only_to_validate<'a>(
        &'a self,
        mut flatten_statements: Vec<&'a Statement>,
        statement: &'a Statement,
    ) -> Vec<&Statement> {
        if let Statement::Class(_) = &statement {
            flatten_statements.push(statement);
        };

        let child_statements: super::ast_child_statements::AstChildStatements = match statement {
            Statement::Namespace(statement) => statement.into(),
            _ => super::ast_child_statements::AstChildStatements { statements: vec![] },
        };

        for statement in &child_statements.statements {
            flatten_statements.append(&mut self.flatten_statements_to_validate(statement));
        }

        flatten_statements
    }
}

impl Rule {
    fn create_reference(
        method: &php_parser_rs::parser::ast::functions::ConcreteMethod,
        rc: &mut file::RC,
    ) {
        let scope_name = &method.name;
        rc.add_reference(scope_name.value.clone());
    }
    fn skip_if_public(
        modifier: &php_parser_rs::parser::ast::modifiers::MethodModifier,
        r: &mut bool,
        modifiers: &Vec<php_parser_rs::parser::ast::modifiers::MethodModifier>,
    ) {
        if let php_parser_rs::parser::ast::modifiers::MethodModifier::Private(_) = *modifier {
            *r = true;
            for i in modifiers {
                match *i {
                    Static(_) => {
                        *r = true;
                    }
                    php_parser_rs::parser::ast::modifiers::MethodModifier::Private(_) => {
                        *r = true;
                    }
                    _ => {}
                };
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e13/detect_dead_code.php", CODE);

        assert!(violations.len().eq(&0));
        assert!(false);
    }
}
