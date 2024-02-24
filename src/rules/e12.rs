use std::str;

use php_parser_rs::parser::ast::classes::ClassStatement;
use php_parser_rs::parser::ast::control_flow::{IfStatement, IfStatementBody};
use php_parser_rs::parser::ast::identifiers::Identifier;
use php_parser_rs::parser::ast::loops::{
    ForStatementBody, ForeachStatement, ForeachStatementBody, WhileStatementBody,
};
use php_parser_rs::parser::ast::operators::ArithmeticOperationExpression;
use php_parser_rs::parser::ast::traits::TraitStatement;
use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::variables::Variable;
use php_parser_rs::parser::ast::{
    namespaces, BlockStatement, Expression, PropertyFetchExpression, Statement,
    StaticPropertyFetchExpression, SwitchStatement,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
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

    fn flatten_statements<'a>(&'a self, statement: &'a Statement) -> Vec<&Statement> {
        let mut flatten_statements: Vec<&Statement> = Vec::new();
        flatten_statements.push(statement);

        match statement {
            Statement::Try(try_statement) => {
                for catch in &try_statement.catches {
                    let CatchBlock { body, .. } = catch;
                    for statement in body {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
            }
            Statement::Class(ClassStatement { body, .. }) => {
                for member in &body.members {
                    if let php_parser_rs::parser::ast::classes::ClassMember::ConcreteMethod(
                        concrete_method,
                    ) = member
                    {
                        for statement in &concrete_method.body.statements {
                            flatten_statements.append(&mut self.flatten_statements(statement));
                        }
                    }
                }
            }
            Statement::Trait(TraitStatement { body, .. }) => {
                for member in &body.members {
                    if let php_parser_rs::parser::ast::traits::TraitMember::ConcreteMethod(
                        concrete_method,
                    ) = member
                    {
                        for statement in &concrete_method.body.statements {
                            flatten_statements.append(&mut self.flatten_statements(statement));
                        }
                    }
                }
            }
            Statement::If(if_statement) => {
                let IfStatement { body, .. } = if_statement;
                {
                    match body {
                        IfStatementBody::Block { statements, .. } => {
                            for statement in statements {
                                flatten_statements.append(&mut self.flatten_statements(statement));
                            }
                        }
                        IfStatementBody::Statement { statement, .. } => {
                            flatten_statements.append(&mut self.flatten_statements(statement))
                        }
                    };
                }
            }
            Statement::While(while_statement) => match &while_statement.body {
                WhileStatementBody::Block { statements, .. } => {
                    for statement in statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                WhileStatementBody::Statement { statement } => {
                    flatten_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::Switch(SwitchStatement { cases, .. }) => {
                for case in cases {
                    for statement in &case.body {
                        flatten_statements.append(&mut self.flatten_statements(statement))
                    }
                }
            }
            Statement::Foreach(ForeachStatement { body, .. }) => match body {
                ForeachStatementBody::Block { statements, .. } => {
                    for statement in statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                ForeachStatementBody::Statement { statement } => {
                    flatten_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::For(for_statement_body) => match &for_statement_body.body {
                ForStatementBody::Block { statements, .. } => {
                    for statement in statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                ForStatementBody::Statement { statement } => {
                    flatten_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::Block(BlockStatement { statements, .. }) => {
                for statement in statements {
                    flatten_statements.append(&mut self.flatten_statements(statement));
                }
            }
            Statement::Namespace(namespace) => match &namespace {
                namespaces::NamespaceStatement::Unbraced(unbraced) => {
                    for statement in &unbraced.statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                namespaces::NamespaceStatement::Braced(braced) => {
                    for statement in &braced.body.statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
            },
            _ => {}
        };

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
