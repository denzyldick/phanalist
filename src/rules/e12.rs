use std::str;

use php_parser_rs::parser::ast::classes::{ClassMember, ClassStatement};
use php_parser_rs::parser::ast::identifiers::Identifier;
use php_parser_rs::parser::ast::operators::ArithmeticOperationExpression;
use php_parser_rs::parser::ast::variables::Variable;
use php_parser_rs::parser::ast::{
    Expression, PropertyFetchExpression, Statement, StaticPropertyFetchExpression,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::ast_child_statements::AstChildStatements;
use crate::rules::do_validate_namespace;

pub static CODE: &str = "E0012";
static DESCRIPTION: &str = "Service properties";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub include_namespaces: Vec<String>,
    pub exclude_namespaces: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            include_namespaces: vec!["App\\Service\\".to_string()],
            exclude_namespaces: vec![],
        }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn set_config(&mut self, json: &Value) {
        if let Ok(settings) = serde_json::from_value(json.to_owned()) {
            self.settings = settings;
        }
    }

    fn do_validate(&self, file: &File) -> bool {
        if let Some(ns) = file.get_fully_qualified_name() {
            return do_validate_namespace(
                ns,
                &self.settings.include_namespaces,
                &self.settings.exclude_namespaces,
            );
        }

        false
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        let mut property: Option<&PropertyFetchExpression> = None;
        let mut static_property: Option<&StaticPropertyFetchExpression> = None;

        if let Statement::Expression(expression) = statement {
            match &expression.expression {
                Expression::AssignmentOperation(assignment) => {
                    match &assignment.left() {
                        Expression::PropertyFetch(property_fetch) => {
                            property = Some(property_fetch);
                        }
                        Expression::StaticPropertyFetch(property_fetch) => {
                            static_property = Some(property_fetch);
                        }
                        _ => {}
                    };
                }
                Expression::ArithmeticOperation(ArithmeticOperationExpression::PostIncrement {
                    left,
                    ..
                }) => match &left.as_ref() {
                    Expression::PropertyFetch(property_fetch) => {
                        property = Some(property_fetch);
                    }
                    Expression::StaticPropertyFetch(property_fetch) => {
                        static_property = Some(property_fetch);
                    }
                    _ => {}
                },
                _ => {}
            };
        };

        if let Some(property) = property {
            if let Expression::Variable(Variable::SimpleVariable(var)) = &property.target.as_ref() {
                if str::from_utf8(&var.name).unwrap() == "$this" {
                    let suggestion = format!("Setting service properties leads to issues with Swoole. Trying to set $this->{} property", Self::get_property_identifier(property));
                    violations.push(self.new_violation(file, suggestion, var.span));
                }
            }
        };

        if let Some(static_property) = static_property {
            if let Expression::Self_ = static_property.target.as_ref() {
                if let Variable::SimpleVariable(var) = &static_property.property {
                    let suggestion = format!("Setting service properties leads to issues with Swoole. Trying to set static {} property", var.name);
                    violations.push(self.new_violation(file, suggestion, var.span));
                }
            }
        }

        violations
    }

    fn travers_statements_to_validate<'a>(
        &'a self,
        mut flatten_statements: Vec<&'a Statement>,
        statement: &'a Statement,
    ) -> Vec<&Statement> {
        if let Statement::Expression(_) = &statement {
            flatten_statements.push(statement);
        };

        let child_statements: AstChildStatements = match statement {
            Statement::Namespace(statement) => statement.into(),
            Statement::Trait(statement) => statement.into(),
            Statement::Class(statement) => {
                crate::rules::e12::Rule::class_statements_ignore_constructor(statement)
            }
            Statement::Block(statement) => statement.into(),
            Statement::If(statement) => statement.into(),
            Statement::Switch(statement) => statement.into(),
            Statement::While(statement) => statement.into(),
            Statement::Foreach(statement) => statement.into(),
            Statement::For(statement) => statement.into(),
            _ => AstChildStatements { statements: vec![] },
        };

        for statement in child_statements.statements {
            flatten_statements.append(&mut self.flatten_statements_to_validate(statement));
        }

        flatten_statements
    }
}

impl Rule {
    fn get_property_identifier(property: &PropertyFetchExpression) -> String {
        if let Expression::Identifier(Identifier::SimpleIdentifier(simple_id)) =
            property.property.as_ref()
        {
            return simple_id.value.to_string();
        }

        "n/a".to_string()
    }

    fn class_statements_ignore_constructor(statement: &ClassStatement) -> AstChildStatements<'_> {
        let mut statements = vec![];

        for member in &statement.body.members {
            if let ClassMember::ConcreteMethod(method) = member {
                for body_statement in &method.body.statements {
                    statements.push(body_statement);
                }
            }
        }

        AstChildStatements { statements }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn define_in_constructor() {
        let violations = analyze_file_for_rule("e12/define_in_constructor.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_in_constructor() {
        let violations = analyze_file_for_rule("e12/set_in_constructor.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_in_method_ignore_ns() {
        let violations = analyze_file_for_rule("e12/set_in_method_ignore_ns.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_local_in_method() {
        let violations = analyze_file_for_rule("e12/set_local_in_method.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn increment_local_in_method() {
        let violations = analyze_file_for_rule("e12/increment_local_in_method.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_local_in_static_method() {
        let violations = analyze_file_for_rule("e12/set_local_in_static_method.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn increment_in_static_method() {
        let violations = analyze_file_for_rule("e12/increment_in_static_method.php", CODE);

        assert!(violations.len().gt(&0));
    }

    #[test]
    fn increment_local_in_static_method() {
        let violations = analyze_file_for_rule("e12/increment_local_in_static_method.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_in_method() {
        let violations = analyze_file_for_rule("e12/set_in_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Swoole. Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn increment_in_method() {
        let violations = analyze_file_for_rule("e12/increment_in_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Swoole. Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_static_method() {
        let violations = analyze_file_for_rule("e12/set_in_static_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Swoole. Trying to set static $counter property".to_string()
        );
    }

    #[test]
    fn set_in_trait_method() {
        let violations = analyze_file_for_rule("e12/set_in_trait_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Swoole. Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_trait_static_method() {
        let violations = analyze_file_for_rule("e12/set_in_trait_static_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Swoole. Trying to set static $counter property".to_string()
        );
    }
}
