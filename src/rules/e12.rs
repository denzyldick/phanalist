use std::str;

use php_parser_rs::parser::ast::arguments::{Argument, NamedArgument, PositionalArgument};
use php_parser_rs::parser::ast::classes::{ClassMember, ClassStatement};
use php_parser_rs::parser::ast::identifiers::Identifier;
use php_parser_rs::parser::ast::operators::ArithmeticOperationExpression;
use php_parser_rs::parser::ast::variables::Variable;
use php_parser_rs::parser::ast::{
    ArrayItem, Expression, MethodCallExpression, PropertyFetchExpression, ShortArrayExpression,
    Statement,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::ast_child_statements::AstChildStatements;
use crate::rules::do_validate_namespace;

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

impl crate::rules::Rule for Rule {
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

        let expression: Option<&Expression> = match statement {
            Statement::Expression(expression) => Some(&expression.expression),
            Statement::Return(return_statement) => {
                if let Some(return_value) = &return_statement.value {
                    match &return_value {
                        Expression::PropertyFetch(_) => None,
                        _ => Some(return_value),
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        let mut flatten_property_expressions: Vec<&Expression> = Vec::new();
        if let Some(expression) = expression {
            flatten_property_expressions =
                self.travers_property_expressions(flatten_property_expressions, expression);
        }

        for property_expression in flatten_property_expressions {
            match property_expression {
                Expression::PropertyFetch(property) => {
                    if let Expression::Variable(Variable::SimpleVariable(var)) =
                        &property.target.as_ref()
                    {
                        if str::from_utf8(&var.name).unwrap() == "$this" {
                            let suggestion = format!("Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->{} property", Self::get_property_identifier(property));
                            violations.push(self.new_violation(file, suggestion, var.span));
                        }
                    }
                }
                Expression::StaticPropertyFetch(static_property) => {
                    if let Expression::Self_ = static_property.target.as_ref() {
                        if let Variable::SimpleVariable(var) = &static_property.property {
                            let suggestion = format!("Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set static {} property", var.name);
                            violations.push(self.new_violation(file, suggestion, var.span));
                        }
                    }
                }
                _ => {}
            };
        }

        violations
    }

    fn travers_statements_to_validate<'a>(
        &'a self,
        mut flatten_statements: Vec<&'a Statement>,
        statement: &'a Statement,
    ) -> Vec<&Statement> {
        if let Statement::Expression(_) | Statement::Return(_) = &statement {
            flatten_statements.push(statement);
        };

        let child_statements: AstChildStatements = match statement {
            Statement::Namespace(statement) => statement.into(),
            Statement::Trait(statement) => statement.into(),
            Statement::Class(statement) => {
                if self.settings.reset_interfaces.is_empty()
                    || !crate::rules::e12::Rule::class_implements(
                        statement,
                        &self.settings.reset_interfaces,
                    )
                {
                    crate::rules::e12::Rule::class_statements_ignore_constructor(statement)
                } else {
                    AstChildStatements { statements: vec![] }
                }
            }
            Statement::Block(statement) => statement.into(),
            Statement::If(statement) => statement.into(),
            Statement::Switch(statement) => statement.into(),
            Statement::While(statement) => statement.into(),
            Statement::Foreach(statement) => statement.into(),
            Statement::For(statement) => statement.into(),
            Statement::Try(statement) => statement.into(),
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

    fn class_implements(statement: &ClassStatement, interfaces: &Vec<String>) -> bool {
        if interfaces.is_empty() {
            return true;
        }

        if let Some(implements) = &statement.implements {
            for class_interface in &implements.interfaces.inner {
                for interface in interfaces {
                    if class_interface.value.to_string().contains(interface) {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn travers_property_expressions<'a>(
        &'a self,
        mut flatten_property_expressions: Vec<&'a Expression>,
        expression: &'a Expression,
    ) -> Vec<&Expression> {
        if self.is_property(expression) {
            flatten_property_expressions.push(expression);
        }

        let expressions = match expression {
            Expression::AssignmentOperation(assignment) => {
                let mut assigment_expressions = vec![assignment.left()];

                let right = assignment.right();
                match right {
                    Expression::PropertyFetch(_) => None,
                    _ => {
                        assigment_expressions.push(right);
                        Some(())
                    }
                };

                assigment_expressions
            },
            Expression::Coalesce(coalesce) => vec![coalesce.lhs.as_ref(), coalesce.rhs.as_ref()],
            Expression::Concat(concat) => vec![concat.left.as_ref(), concat.right.as_ref()],
            Expression::Parenthesized(parenthesized) => vec![parenthesized.expr.as_ref()],
            Expression::ArithmeticOperation(arithmetic) => match arithmetic {
                ArithmeticOperationExpression::PreIncrement { right, .. } => vec![right.as_ref()],
                ArithmeticOperationExpression::PostIncrement { left, .. } => vec![left.as_ref()],
                _ => vec![],
            },
            Expression::ShortArray(short_array) => self.get_short_array_expressions(short_array),
            Expression::MethodCall(method_call) => self.get_method_expressions(method_call),
            _ => vec![],
        };

        for child_expression in expressions {
            flatten_property_expressions =
                self.travers_property_expressions(flatten_property_expressions, child_expression);
        }

        flatten_property_expressions
    }

    fn get_short_array_expressions<'a>(
        &'a self,
        short_array: &'a ShortArrayExpression,
    ) -> Vec<&Expression> {
        let mut expressions = vec![];

        for item in short_array.items.iter().clone() {
            let mut item_expressions = match &item {
                ArrayItem::KeyValue { key, value, .. } => vec![key, value],
                ArrayItem::ReferencedKeyValue { key, value, .. } => vec![key, value],
                ArrayItem::ReferencedValue { value, .. } => vec![value],
                ArrayItem::SpreadValue { value, .. } => vec![value],
                ArrayItem::Value { value } => vec![value],
                ArrayItem::Skipped => vec![],
            };

            expressions.append(&mut item_expressions);
        }

        expressions
    }

    fn get_method_expressions<'a>(
        &'a self,
        method_call: &'a MethodCallExpression,
    ) -> Vec<&Expression> {
        let mut expressions = vec![];

        for argument in method_call.arguments.iter().clone() {
            let argument_expression = match &argument {
                Argument::Positional(PositionalArgument { value, .. }) => value,
                Argument::Named(NamedArgument { value, .. }) => value,
            };

            match &argument_expression {
                Expression::PropertyFetch(_) => {}
                _ => expressions.push(argument_expression),
            };
        }

        expressions
    }

    fn is_property(&self, expression: &Expression) -> bool {
        matches!(
            expression,
            Expression::PropertyFetch(_) | Expression::StaticPropertyFetch(_)
        )
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
    fn set_in_method_wth_reset_interface() {
        let violations = analyze_file_for_rule("e12/set_in_method_wth_reset_interface.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn read_in_method_call() {
        let violations = analyze_file_for_rule("e12/read_in_method_call.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn read_in_return() {
        let violations = analyze_file_for_rule("e12/read_in_return.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn read_local_set() {
        let violations = analyze_file_for_rule("e12/read_local_set.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_in_method() {
        let violations = analyze_file_for_rule("e12/set_in_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn increment_in_method() {
        let violations = analyze_file_for_rule("e12/increment_in_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_static_method() {
        let violations = analyze_file_for_rule("e12/set_in_static_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set static $counter property".to_string()
        );
    }

    #[test]
    fn set_in_trait_method() {
        let violations = analyze_file_for_rule("e12/set_in_trait_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_trait_static_method() {
        let violations = analyze_file_for_rule("e12/set_in_trait_static_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set static $counter property".to_string()
        );
    }

    #[test]
    fn set_in_method_try() {
        let violations = analyze_file_for_rule("e12/set_in_method_try.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_method_catch() {
        let violations = analyze_file_for_rule("e12/set_in_method_catch.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_method_finalliy() {
        let violations = analyze_file_for_rule("e12/set_in_method_finalliy.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_assigment() {
        let violations = analyze_file_for_rule("e12/set_in_assigment.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_return() {
        let violations = analyze_file_for_rule("e12/set_in_return.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_return_nested_method() {
        let violations = analyze_file_for_rule("e12/set_in_return_nested_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn set_in_null_coalescing(){
        let violations = analyze_file_for_rule("e12/set_in_null_coalescing.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Shared Memory Model (FrankenPHP/Swoole/RoadRunner). Trying to set $this->counter property".to_string()
        );
        
    }


}
