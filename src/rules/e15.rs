use std::collections::{HashMap, HashSet};

use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0015";
static DESCRIPTION: &str = "Lack of Cohesion of Methods (LCOM4)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub threshold: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { threshold: 1 }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

struct Dsu {
    parent: Vec<usize>,
    count: usize,
}

impl Dsu {
    fn new(n: usize) -> Self {
        Dsu {
            parent: (0..n).collect(),
            count: n,
        }
    }

    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut curr = i;
        while self.parent[curr] != root {
            let next = self.parent[curr];
            self.parent[curr] = root;
            curr = next;
        }
        root
    }

    fn union(&mut self, i: usize, j: usize) {
        let root_i = self.find(i);
        let root_j = self.find(j);
        if root_i != root_j {
            self.parent[root_i] = root_j;
            self.count -= 1;
        }
    }
}

/// Represents a "node" in the cohesion graph.
/// Could be a method or a property hook.
enum MethodNode<'a> {
    Method(&'a Method<'a>),
    Hook(&'a PropertyHook<'a>, String), // hook, property_name
}

impl<'a> MethodNode<'a> {
    fn name(&self) -> String {
        match self {
            MethodNode::Method(m) => String::from_utf8_lossy(m.name.value).into_owned(),
            MethodNode::Hook(h, prop) => format!("{}::{}", prop, String::from_utf8_lossy(h.name.value)),
        }
    }

    fn body_statements(&self) -> Option<&'a [Statement<'a>]> {
        match self {
            MethodNode::Method(m) => match &m.body {
                MethodBody::Concrete(block) => Some(block.statements.iter().as_slice()),
                _ => None,
            },
            MethodNode::Hook(h, _) => match &h.body {
                PropertyHookBody::Concrete(PropertyHookConcreteBody::Block(block)) => {
                    Some(block.statements.iter().as_slice())
                }
                _ => None,
            },
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

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        }
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            let mut nodes = Vec::new();
            let mut property_names = HashSet::new();

            for member in class.members.iter() {
                match member {
                    ClassLikeMember::Method(method) => {
                        let name = String::from_utf8_lossy(method.name.value).into_owned();
                        if !name.starts_with("__") {
                            nodes.push(MethodNode::Method(method));
                        }
                    }
                    ClassLikeMember::Property(prop) => {
                        let is_static = prop
                            .modifiers()
                            .iter()
                            .any(|m| matches!(m, Modifier::Static(_)));
                        if !is_static {
                            let vars = prop.variables();
                            for var in vars {
                                let prop_name = String::from_utf8_lossy(var.name).into_owned();
                                property_names.insert(prop_name.clone());
                            }

                            if let Property::Hooked(h) = prop {
                                let prop_name = String::from_utf8_lossy(h.item.variable().name).into_owned();
                                for hook in h.hook_list.hooks.iter() {
                                    nodes.push(MethodNode::Hook(hook, prop_name.clone()));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            if nodes.is_empty() {
                return violations;
            }

            let n = nodes.len();
            let mut dsu = Dsu::new(n);
            let mut property_to_methods: HashMap<String, Vec<usize>> = HashMap::new();
            let mut method_to_index: HashMap<String, usize> = HashMap::new();

            for (i, node) in nodes.iter().enumerate() {
                if let MethodNode::Method(_) = node {
                    method_to_index.insert(node.name(), i);
                }
            }

            for (i, node) in nodes.iter().enumerate() {
                let mut used_props = HashSet::new();
                let mut called_methods = HashSet::new();

                if let Some(stmts) = node.body_statements() {
                    for stmt in stmts {
                        for s in self.flatten_statements_to_validate(stmt) {
                            self.scan_statement(
                                s,
                                &property_names,
                                &mut used_props,
                                &mut called_methods,
                            );
                        }
                    }
                } else if let MethodNode::Hook(h, _) = node {
                    if let PropertyHookBody::Concrete(PropertyHookConcreteBody::Expression(expr)) =
                        &h.body
                    {
                        self.scan_expression(
                            expr.expression,
                            &property_names,
                            &mut used_props,
                            &mut called_methods,
                        );
                    }
                }

                for prop in used_props {
                    property_to_methods.entry(prop).or_default().push(i);
                }

                for called in called_methods {
                    if let Some(&j) = method_to_index.get(&called) {
                        dsu.union(i, j);
                    }
                }
            }

            for methods_using_prop in property_to_methods.values() {
                if let Some(&first) = methods_using_prop.first() {
                    for &next in methods_using_prop.iter().skip(1) {
                        dsu.union(first, next);
                    }
                }
            }

            let lcom4 = dsu.count;
            if lcom4 > self.settings.threshold {
                let message = Message::new(
                    "E0015:low-cohesion",
                    "Class \"{class}\" has low cohesion (LCOM4 = {lcom4}). Consider splitting it into {count} smaller classes.",
                )
                .arg("class", String::from_utf8_lossy(class.name.value).to_string())
                .arg("lcom4", lcom4.to_string())
                .arg("count", lcom4.to_string());
                violations.push(self.new_violation(file, message, class.span()));
            }
        }

        violations
    }
}

impl Rule {
    fn is_this(&self, expr: &Expression<'_>) -> bool {
        if let Expression::Variable(Variable::Direct(d)) = expr {
            return d.name == b"$this";
        }
        false
    }

    fn scan_statement(
        &self,
        stmt: &Statement<'_>,
        property_names: &HashSet<String>,
        used_props: &mut HashSet<String>,
        called_methods: &mut HashSet<String>,
    ) {
        match stmt {
            Statement::Expression(s) => {
                self.scan_expression(s.expression, property_names, used_props, called_methods)
            }
            Statement::Return(s) => {
                if let Some(expr) = s.value {
                    self.scan_expression(expr, property_names, used_props, called_methods);
                }
            }
            Statement::Echo(s) => {
                for expr in s.values.iter() {
                    self.scan_expression(expr, property_names, used_props, called_methods);
                }
            }
            _ => {}
        }
    }

    fn scan_expression(
        &self,
        expr: &Expression<'_>,
        property_names: &HashSet<String>,
        used_props: &mut HashSet<String>,
        called_methods: &mut HashSet<String>,
    ) {
        match expr {
            Expression::Call(call) => match call {
                Call::Method(m) => {
                    if self.is_this(m.object) {
                        if let ClassLikeMemberSelector::Identifier(id) = &m.method {
                            called_methods.insert(String::from_utf8_lossy(id.value).into_owned());
                        }
                    }
                    self.scan_expression(m.object, property_names, used_props, called_methods);
                    for arg in m.argument_list.arguments.iter() {
                        self.scan_expression(
                            arg.value(),
                            property_names,
                            used_props,
                            called_methods,
                        );
                    }
                }
                Call::NullSafeMethod(m) => {
                    if self.is_this(m.object) {
                        if let ClassLikeMemberSelector::Identifier(id) = &m.method {
                            called_methods.insert(String::from_utf8_lossy(id.value).into_owned());
                        }
                    }
                    self.scan_expression(m.object, property_names, used_props, called_methods);
                    for arg in m.argument_list.arguments.iter() {
                        self.scan_expression(
                            arg.value(),
                            property_names,
                            used_props,
                            called_methods,
                        );
                    }
                }
                Call::Function(f) => {
                    self.scan_expression(f.function, property_names, used_props, called_methods);
                    for arg in f.argument_list.arguments.iter() {
                        self.scan_expression(
                            arg.value(),
                            property_names,
                            used_props,
                            called_methods,
                        );
                    }
                }
                Call::StaticMethod(m) => {
                    self.scan_expression(m.class, property_names, used_props, called_methods);
                    for arg in m.argument_list.arguments.iter() {
                        self.scan_expression(
                            arg.value(),
                            property_names,
                            used_props,
                            called_methods,
                        );
                    }
                }
            },
            Expression::Access(access) => match access {
                Access::Property(p) => {
                    if self.is_this(p.object) {
                        if let ClassLikeMemberSelector::Identifier(id) = &p.property {
                            let name = format!("${}", String::from_utf8_lossy(id.value));
                            if property_names.contains(&name) {
                                used_props.insert(name);
                            }
                        }
                    }
                    self.scan_expression(p.object, property_names, used_props, called_methods);
                }
                Access::NullSafeProperty(p) => {
                    if self.is_this(p.object) {
                        if let ClassLikeMemberSelector::Identifier(id) = &p.property {
                            let name = format!("${}", String::from_utf8_lossy(id.value));
                            if property_names.contains(&name) {
                                used_props.insert(name);
                            }
                        }
                    }
                    self.scan_expression(p.object, property_names, used_props, called_methods);
                }
                Access::ClassConstant(c) => {
                    self.scan_expression(c.class, property_names, used_props, called_methods);
                }
                Access::StaticProperty(p) => {
                    self.scan_expression(p.class, property_names, used_props, called_methods);
                }
            },
            Expression::Binary(bin) => {
                self.scan_expression(bin.lhs, property_names, used_props, called_methods);
                self.scan_expression(bin.rhs, property_names, used_props, called_methods);
            }
            Expression::UnaryPrefix(un) => {
                self.scan_expression(un.operand, property_names, used_props, called_methods);
            }
            Expression::UnaryPostfix(un) => {
                self.scan_expression(un.operand, property_names, used_props, called_methods);
            }
            Expression::Assignment(ass) => {
                self.scan_expression(ass.lhs, property_names, used_props, called_methods);
                self.scan_expression(ass.rhs, property_names, used_props, called_methods);
            }
            Expression::Parenthesized(p) => {
                self.scan_expression(p.expression, property_names, used_props, called_methods);
            }
            Expression::Array(a) => {
                for element in a.elements.iter() {
                    if let Some(val) = element.get_value() {
                        self.scan_expression(val, property_names, used_props, called_methods);
                    }
                    if let Some(key) = element.get_key() {
                        self.scan_expression(key, property_names, used_props, called_methods);
                    }
                }
            }
            Expression::Closure(c) => {
                for stmt in c.body.statements.iter() {
                    for s in self.flatten_statements_to_validate(stmt) {
                        self.scan_statement(s, property_names, used_props, called_methods);
                    }
                }
            }
            Expression::ArrowFunction(c) => {
                self.scan_expression(c.expression, property_names, used_props, called_methods);
            }
            Expression::Match(m) => {
                self.scan_expression(m.expression, property_names, used_props, called_methods);
                for arm in m.arms.iter() {
                    match arm {
                        MatchArm::Expression(a) => {
                            for cond in a.conditions.iter() {
                                self.scan_expression(
                                    cond,
                                    property_names,
                                    used_props,
                                    called_methods,
                                );
                            }
                            self.scan_expression(
                                a.expression,
                                property_names,
                                used_props,
                                called_methods,
                            );
                        }
                        MatchArm::Default(a) => {
                            self.scan_expression(
                                a.expression,
                                property_names,
                                used_props,
                                called_methods,
                            );
                        }
                    }
                }
            }
            Expression::AnonymousClass(_) => {
                // Stop! Do not traverse into anonymous classes as they have their own scope
            }
            Expression::Conditional(t) => {
                self.scan_expression(t.condition, property_names, used_props, called_methods);
                if let Some(then_expr) = t.then {
                    self.scan_expression(then_expr, property_names, used_props, called_methods);
                }
                self.scan_expression(t.r#else, property_names, used_props, called_methods);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn test_cohesive_class() {
        let violations = analyze_file_for_rule("e15/cohesive.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_non_cohesive_class() {
        let violations = analyze_file_for_rule("e15/non_cohesive.php", CODE);
        assert!(violations.len() > 0);
        assert!(violations[0].message.render().contains("LCOM4 = 2"));
    }

    #[test]
    fn test_closure_cohesion() {
        let violations = analyze_file_for_rule("e15/closure_cohesion.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_anonymous_class_isolation() {
        let violations = analyze_file_for_rule("e15/anonymous_class.php", CODE);
        assert!(violations.len() > 0);
    }
}
