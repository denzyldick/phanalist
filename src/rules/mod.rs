use colored::Colorize;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use mago_span::Span;
use mago_syntax::cst::*;
use serde_json::Value;

use crate::config::Config;
use crate::file::File;
use crate::resource::{ResourceClass, ResourceSettings};
use crate::results::{Message, Violation};
use crate::verification::{FindingClass, Settings};
pub mod ast_support;
pub mod e0;
pub mod e1;
pub mod e10;
pub mod e11;
pub mod e12;
pub mod e13;
pub mod e14;
pub mod e15;
pub mod e16;
pub mod e17;
pub mod e18;
pub mod e19;
pub mod e2;
pub mod e20;
pub mod e21;
pub mod e22;
pub mod e23;
pub mod e24;
pub mod e25;
pub mod e26;
pub mod e27;
pub mod e28;
pub mod e29;
pub mod e3;
pub mod e30;
pub mod e31;
pub mod e4;
pub mod e47;
pub mod e48;
pub mod e49;
pub mod e5;
pub mod e50;
pub mod e51;
pub mod e52;
pub mod e53;
pub mod e54;
pub mod e55;
pub mod e56;
pub mod e57;
pub mod e58;
pub mod e59;
pub mod e6;
pub mod e7;
pub mod e8;
pub mod e9;
pub mod resource;
pub mod verification;

pub trait Rule: Sync {
    /// Optional hook for cross-file type resolution or indexing.
    /// Called once for every file before main validation pass.
    fn index_file(&self, _file: &File<'_>) {}

    /// Optional hook called before a project-wide indexing pass. Rules that
    /// accumulate cross-file state should clear it here.
    fn begin_index(&self) {}

    /// Optional hook called after every file has been indexed. Rules that need
    /// a fixed point or cross-file resolution finalize their state here.
    fn finalize_index(&self) {}

    /// Optional hook for dropping state when a file is removed or closed.
    fn remove_file(&self, _path: &Path) {}

    /// The verification finding class implemented by this rule, if any.
    fn verification_class(&self) -> Option<FindingClass> {
        None
    }

    /// Verification settings for this rule. Verification rules share one
    /// analysis pass when their settings are identical.
    fn verification_settings(&self) -> Option<Settings> {
        None
    }

    /// The resource finding class implemented by this rule, if any.
    fn resource_class(&self) -> Option<ResourceClass> {
        None
    }

    /// Resource settings for this rule. Resource rules share one analysis pass
    /// when their settings are identical.
    fn resource_settings(&self) -> Option<ResourceSettings> {
        None
    }

    /// Optional hook for whole-file validation. Most rules validate each
    /// top-level statement through `validate`; verification rules need the
    /// entire file (and its CFG) at once and override this instead.
    fn validate_file(&self, _file: &File<'_>) -> Option<Vec<Violation>> {
        None
    }

    /// Would be a good idea to have default implementation which extracts the code from struct name
    /// Haven't found a way to implement it
    fn get_code(&self) -> String;

    fn description(&self) -> String {
        String::from("")
    }
    /// Every rule has a detailed explanation.
    /// They are written in markdown and are located in
    /// the examples directory.
    fn get_detailed_explanation(&self) -> Option<String> {
        let code = self.get_code();
        let rule_number = code.trim_start_matches('E').trim_start_matches('0');
        let rule_number = if rule_number.is_empty() {
            "0"
        } else {
            rule_number
        };

        let relative_path = PathBuf::from(format!(
            "./src/rules/examples/e{rule_number}/e{rule_number}.md"
        ));
        let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(relative_path.strip_prefix("./").unwrap_or(&relative_path));

        for path in [relative_path, manifest_path] {
            if path.exists() {
                let text = fs::read_to_string(path);
                if let Ok(text) = text {
                    return Some(text.to_string());
                }
            }
        }
        None
    }

    fn set_config(&mut self, _json: &Value) {}

    fn read_config(&mut self, config: &Config) {
        let code = self.get_code();
        if let Some(rule_config) = config.rules.get(&code) {
            self.set_config(rule_config);
        }
    }

    fn output_error(&self, e: Box<dyn Error>) {
        println!(
            "{}",
            format!(
                "Unable to parse config for rule #{}, so default values will be used. Parsing error: {}",
                self.get_code(),
                e
            )
            .red()
            .bold()
        );
    }

    fn do_validate(&self, file: &File<'_>) -> bool {
        file.get_fully_qualified_name().is_some()
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation>;

    fn new_violation(&self, file: &File<'_>, message: Message, span: Span) -> Violation {
        let start_line = file.line_number(span.start.offset);
        let start_column = file.column_number(span.start.offset);
        let end_line = file.line_number(span.end.offset);
        let end_column = file.column_number(span.end.offset);

        let line = file
            .lines
            .get(start_line.saturating_sub(1))
            .cloned()
            .unwrap_or_default();

        Violation {
            rule: self.get_code(),
            line,
            message,
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    fn flatten_statements_to_validate<'a>(
        &'a self,
        statement: &'a Statement<'a>,
    ) -> Vec<&'a Statement<'a>> {
        let mut flatten_statements: Vec<&Statement<'a>> = Vec::new();
        self.traverse_statements_to_validate(&mut flatten_statements, statement);
        flatten_statements
    }

    fn traverse_statements_to_validate<'a>(
        &'a self,
        flatten_statements: &mut Vec<&'a Statement<'a>>,
        statement: &'a Statement<'a>,
    ) {
        flatten_statements.push(statement);

        match statement {
            Statement::Block(block) => {
                for s in block.statements.iter() {
                    self.traverse_statements_to_validate(flatten_statements, s);
                }
            }
            Statement::If(if_stmt) => match &if_stmt.body {
                IfBody::Statement(body) => {
                    self.traverse_statements_to_validate(flatten_statements, body.statement);
                    for clauses in body.else_if_clauses.iter() {
                        self.traverse_statements_to_validate(flatten_statements, clauses.statement);
                    }
                    if let Some(else_clause) = &body.else_clause {
                        self.traverse_statements_to_validate(
                            flatten_statements,
                            else_clause.statement,
                        );
                    }
                }
                IfBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        self.traverse_statements_to_validate(flatten_statements, s);
                    }
                    for clauses in body.else_if_clauses.iter() {
                        for s in clauses.statements.iter() {
                            self.traverse_statements_to_validate(flatten_statements, s);
                        }
                    }
                    if let Some(else_clause) = &body.else_clause {
                        for s in else_clause.statements.iter() {
                            self.traverse_statements_to_validate(flatten_statements, s);
                        }
                    }
                }
            },
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(body) => {
                    self.traverse_statements_to_validate(flatten_statements, body);
                }
                WhileBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        self.traverse_statements_to_validate(flatten_statements, s);
                    }
                }
            },
            Statement::DoWhile(do_while_stmt) => {
                self.traverse_statements_to_validate(flatten_statements, do_while_stmt.statement);
            }
            Statement::Foreach(foreach_stmt) => match &foreach_stmt.body {
                ForeachBody::Statement(body) => {
                    self.traverse_statements_to_validate(flatten_statements, body);
                }
                ForeachBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        self.traverse_statements_to_validate(flatten_statements, s);
                    }
                }
            },
            Statement::For(for_stmt) => match &for_stmt.body {
                ForBody::Statement(body) => {
                    self.traverse_statements_to_validate(flatten_statements, body);
                }
                ForBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        self.traverse_statements_to_validate(flatten_statements, s);
                    }
                }
            },
            Statement::Try(try_stmt) => {
                for s in try_stmt.block.statements.iter() {
                    self.traverse_statements_to_validate(flatten_statements, s);
                }
                for catch in try_stmt.catch_clauses.iter() {
                    for s in catch.block.statements.iter() {
                        self.traverse_statements_to_validate(flatten_statements, s);
                    }
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    for s in finally.block.statements.iter() {
                        self.traverse_statements_to_validate(flatten_statements, s);
                    }
                }
            }
            Statement::Namespace(namespace) => {
                for s in namespace.statements().iter() {
                    self.traverse_statements_to_validate(flatten_statements, s);
                }
            }
            Statement::Class(class) => {
                for member in class.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            for s in block.statements.iter() {
                                self.traverse_statements_to_validate(flatten_statements, s);
                            }
                        }
                    }
                }
            }
            Statement::Interface(interface) => {
                for member in interface.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            for s in block.statements.iter() {
                                self.traverse_statements_to_validate(flatten_statements, s);
                            }
                        }
                    }
                }
            }
            Statement::Trait(t) => {
                for member in t.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            for s in block.statements.iter() {
                                self.traverse_statements_to_validate(flatten_statements, s);
                            }
                        }
                    }
                }
            }
            Statement::Enum(e) => {
                for member in e.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            for s in block.statements.iter() {
                                self.traverse_statements_to_validate(flatten_statements, s);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn do_validate_namespace(
    ns: String,
    include: &Vec<String>,
    exclude: &Vec<String>,
) -> bool {
    for exclude_ns in exclude {
        if ns.contains(exclude_ns) {
            return false;
        }
    }

    if !include.is_empty() {
        for include_ns in include {
            if ns.contains(include_ns) {
                return true;
            }
        }

        return false;
    }

    true
}

fn add_rule(rules: &mut HashMap<String, Box<dyn Rule>>, rule: Box<dyn Rule>) {
    rules.insert(rule.get_code(), rule as Box<dyn Rule>);
}

pub fn all_rules() -> HashMap<String, Box<dyn Rule>> {
    let mut rules: HashMap<String, Box<dyn Rule>> = HashMap::new();

    add_rule(&mut rules, Box::new(e0::Rule {}));
    add_rule(&mut rules, Box::new(e1::Rule {}));
    add_rule(&mut rules, Box::new(e2::Rule {}));
    add_rule(&mut rules, Box::new(e3::Rule {}));
    add_rule(&mut rules, Box::new(e4::Rule {}));
    add_rule(&mut rules, Box::new(e5::Rule {}));
    add_rule(&mut rules, Box::new(e6::Rule {}));
    add_rule(&mut rules, Box::default() as Box<e7::Rule>);
    add_rule(&mut rules, Box::new(e8::Rule {}));
    add_rule(&mut rules, Box::default() as Box<e9::Rule>);
    add_rule(&mut rules, Box::default() as Box<e10::Rule>);
    add_rule(&mut rules, Box::new(e11::Rule {}));
    add_rule(&mut rules, Box::default() as Box<e12::Rule>);
    add_rule(&mut rules, Box::new(e13::Rule {}));
    add_rule(&mut rules, Box::new(e14::Rule::default()));
    add_rule(&mut rules, Box::default() as Box<e15::Rule>);
    add_rule(&mut rules, Box::default() as Box<e16::Rule>);
    add_rule(&mut rules, Box::default() as Box<e17::Rule>);
    add_rule(&mut rules, Box::default() as Box<e18::Rule>);
    add_rule(&mut rules, Box::default() as Box<e19::Rule>);
    add_rule(&mut rules, Box::default() as Box<e20::Rule>);
    add_rule(&mut rules, Box::default() as Box<e21::Rule>);
    add_rule(&mut rules, Box::default() as Box<e22::Rule>);
    add_rule(&mut rules, Box::default() as Box<e23::Rule>);
    add_rule(&mut rules, Box::default() as Box<e24::Rule>);
    add_rule(&mut rules, Box::default() as Box<e25::Rule>);
    add_rule(&mut rules, Box::default() as Box<e26::Rule>);
    add_rule(&mut rules, Box::default() as Box<e27::Rule>);
    add_rule(&mut rules, Box::default() as Box<e28::Rule>);
    add_rule(&mut rules, Box::default() as Box<e29::Rule>);
    add_rule(&mut rules, Box::default() as Box<e30::Rule>);
    for class in FindingClass::ALL {
        add_rule(&mut rules, Box::new(verification::Rule::new(class)));
    }
    for class in ResourceClass::all() {
        add_rule(&mut rules, Box::new(resource::Rule::new(class)));
    }
    add_rule(&mut rules, Box::new(e47::Rule::default()));
    add_rule(&mut rules, Box::new(e48::Rule::default()));
    add_rule(&mut rules, Box::new(e49::Rule::default()));
    add_rule(&mut rules, Box::new(e50::Rule::default()));
    add_rule(&mut rules, Box::new(e51::Rule::default()));
    add_rule(&mut rules, Box::new(e52::Rule::default()));
    add_rule(&mut rules, Box::new(e53::Rule::default()));
    add_rule(&mut rules, Box::new(e54::Rule::default()));
    add_rule(&mut rules, Box::new(e55::Rule::default()));
    add_rule(&mut rules, Box::new(e56::Rule::default()));
    add_rule(&mut rules, Box::new(e57::Rule::default()));
    add_rule(&mut rules, Box::new(e58::Rule::default()));
    add_rule(&mut rules, Box::new(e59::Rule::default()));

    rules
}

#[cfg(test)]
mod tests {
    use mago_allocator::prelude::LocalArena;
    use std::fs;

    use crate::analyse::Analyse;

    use super::*;

    pub(crate) fn analyze_file_for_rule(path: &str, rule_code: &str) -> Vec<Violation> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("rules")
            .join("examples")
            .join(path);
        let content = fs::read_to_string(&path).unwrap();
        let arena = LocalArena::new();
        let mut file = File::new(&arena, path, content);

        let enabled_rules = if rule_code == "E0031" {
            FindingClass::ALL
                .iter()
                .map(|class| String::from(class.code()))
                .collect()
        } else {
            vec![rule_code.to_string()]
        };
        let config = Config {
            enabled_rules,
            ..Default::default()
        };
        let analyse = Analyse::new(&config);

        analyse.begin_index();
        analyse.index_file(&file);
        analyse.finalize_index();

        analyse.analyse_file(&mut file, false).0
    }

    fn get_ns() -> String {
        "App\\Service\\Search".to_string()
    }

    #[test]
    fn do_validate_namespace_empty_include_and_exclude() {
        assert!(do_validate_namespace(get_ns(), &vec![], &vec![]));
    }

    #[test]
    fn do_validate_namespace_include_contains() {
        assert!(do_validate_namespace(
            get_ns(),
            &vec!["\\Service\\".to_string()],
            &vec![]
        ));
    }

    #[test]
    fn do_validate_namespace_include_not_contains() {
        assert!(!do_validate_namespace(
            get_ns(),
            &vec!["\\Service2\\".to_string()],
            &vec![]
        ));
    }

    #[test]
    fn do_validate_namespace_exclude_contains() {
        assert!(!do_validate_namespace(
            get_ns(),
            &vec![],
            &vec!["\\Service\\".to_string()],
        ));
    }

    #[test]
    fn do_validate_namespace_exclude_not_contains() {
        assert!(do_validate_namespace(
            get_ns(),
            &vec![],
            &vec!["\\Service2\\".to_string()],
        ));
    }

    #[test]
    fn do_validate_namespace_include_contains_exclude_contains() {
        let namespaces = &vec!["\\Service\\".to_string()];
        assert!(!do_validate_namespace(get_ns(), namespaces, namespaces));
    }

    #[test]
    fn default_config_registers_each_verification_class() {
        let analyse = Analyse::new(&Config::default());
        for class in FindingClass::ALL {
            assert!(analyse.rules.contains_key(class.code()), "{}", class.code());
        }
    }

    #[test]
    fn default_config_registers_each_resource_class() {
        let analyse = Analyse::new(&Config::default());
        for class in ResourceClass::all() {
            assert!(analyse.rules.contains_key(class.code()), "{}", class.code());
        }
    }

    #[test]
    fn enabling_one_verification_class_enables_only_that_class() {
        let config = Config {
            enabled_rules: vec![String::from("E0031")],
            ..Default::default()
        };
        let analyse = Analyse::new(&config);
        assert!(analyse.rules.contains_key("E0031"));
        for class in FindingClass::ALL {
            if class != FindingClass::Division {
                assert!(
                    !analyse.rules.contains_key(class.code()),
                    "{}",
                    class.code()
                );
            }
        }
    }

    #[test]
    fn disabling_one_verification_class_disables_only_that_class() {
        let config = Config {
            disable_rules: vec![String::from("E0031")],
            ..Default::default()
        };
        let analyse = Analyse::new(&config);
        assert!(!analyse.rules.contains_key("E0031"));
        for class in FindingClass::ALL {
            if class != FindingClass::Division {
                assert!(analyse.rules.contains_key(class.code()), "{}", class.code());
            }
        }
    }

    #[test]
    fn explicit_verification_class_enables_only_itself() {
        let config = Config {
            enabled_rules: vec![String::from("E0036")],
            ..Default::default()
        };
        let analyse = Analyse::new(&config);
        assert!(analyse.rules.contains_key("E0036"));
        assert!(!analyse.rules.contains_key("E0031"));
        assert!(!analyse.rules.contains_key("E0037"));
    }

    #[test]
    fn class_specific_rule_only_reports_its_class() {
        let violations = analyze_file_for_rule("e31/list_bounds_vulnerable.php", "E0036");
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == "E0036"));

        let violations = analyze_file_for_rule("e31/division_by_zero_literal.php", "E0031");
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == "E0031"));
    }

    #[test]
    fn every_rule_has_a_detailed_explanation() {
        for (code, rule) in all_rules() {
            assert!(
                rule.get_detailed_explanation().is_some(),
                "missing documentation for {code}"
            );
        }
    }

    #[test]
    fn new_rule_docs_have_required_sections() {
        let mut rules = all_rules();
        for code in [
            "E0050", "E0051", "E0052", "E0053", "E0054", "E0055", "E0056", "E0057", "E0058",
            "E0059",
        ] {
            let rule = rules
                .remove(code)
                .unwrap_or_else(|| panic!("missing {code}"));
            let doc = rule.get_detailed_explanation().unwrap();
            for section in [
                "**In plain English:**",
                "## Bad",
                "## Good",
                "## What the rule checks",
                "## When it does not report",
                "## How to fix",
                "## Configuration",
            ] {
                assert!(
                    doc.contains(section),
                    "{code} documentation is missing {section}"
                );
            }
        }
    }

    #[test]
    fn every_verification_rule_has_a_detailed_explanation() {
        for class in FindingClass::ALL {
            let rule = verification::Rule::new(class);
            assert!(
                rule.get_detailed_explanation().is_some(),
                "missing documentation for {}",
                class.code()
            );
        }
    }

    #[test]
    fn every_resource_rule_has_a_detailed_explanation() {
        for class in ResourceClass::all() {
            let rule = super::resource::Rule::new(class);
            assert!(
                rule.get_detailed_explanation().is_some(),
                "missing documentation for {}",
                class.code()
            );
        }
    }

    #[test]
    fn validate_markdown() {
        let rule = e1::Rule {};
        let markdown = rule.get_detailed_explanation().unwrap();
        let e1_markdown = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/rules/examples/e1/e1.md"),
        )
        .unwrap();
        assert_eq!(e1_markdown, markdown);
    }
}
