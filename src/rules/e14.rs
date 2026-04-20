use std::collections::HashMap;

use mago_span::{HasSpan, Span};
use mago_syntax::ast::*;

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

static CODE: &str = "E0014";
static DESCRIPTION: &str =
    "Law of Demeter violation. Method chaining should be avoided unless returning the same object type.";

/// Contains all collected types: methods, properties, and interface identities.
#[derive(Default, Clone)]
pub struct TypeRegistry {
    pub methods: HashMap<String, HashMap<String, String>>,
    pub properties: HashMap<String, HashMap<String, String>>,
    pub interfaces: std::collections::HashSet<String>,
}

/// Maps local variable name → resolved type string
type VarTypes = HashMap<String, String>;

#[derive(Default)]
pub struct Rule {
    pub global_registry: std::sync::Mutex<TypeRegistry>,
}

impl crate::rules::Rule for Rule {
    fn index_file(&self, file: &File<'_>) {
        if let Some(program) = file.ast {
            let mut file_registry = TypeRegistry::default();
            for statement in program.statements.iter() {
                self.collect_types(statement, &mut file_registry);
            }
            if let Ok(mut global) = self.global_registry.lock() {
                for (class_name, methods) in file_registry.methods {
                    global
                        .methods
                        .entry(class_name)
                        .or_default()
                        .extend(methods);
                }
                for (class_name, props) in file_registry.properties {
                    global
                        .properties
                        .entry(class_name)
                        .or_default()
                        .extend(props);
                }
                global.interfaces.extend(file_registry.interfaces);
            }
        }
    }

    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        true
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        // Build the type registry from ALL statements in the file so that cross-file
        // references (e.g. a class using a trait defined elsewhere in the same file)
        // are resolvable. Fall back to building from just this statement if no AST.
        let mut registry = TypeRegistry::default();
        if let Some(program) = file.ast {
            for s in program.statements.iter() {
                self.collect_types(s, &mut registry);
            }
        } else {
            match statement {
                Statement::Namespace(ns) => {
                    for s in ns.statements().iter() {
                        self.collect_types(s, &mut registry);
                    }
                }
                _ => {
                    self.collect_types(statement, &mut registry);
                }
            }
        }

        if let Ok(global) = self.global_registry.lock() {
            for (class_name, methods) in global.methods.iter() {
                registry
                    .methods
                    .entry(class_name.clone())
                    .or_default()
                    .extend(methods.clone());
            }
            for (class_name, props) in global.properties.iter() {
                registry
                    .properties
                    .entry(class_name.clone())
                    .or_default()
                    .extend(props.clone());
            }
            registry.interfaces.extend(global.interfaces.clone());
        }

        match statement {
            Statement::Namespace(ns) => {
                for s in ns.statements().iter() {
                    self.validate_statement(file, s, &registry, &mut violations);
                }
            }
            _ => {
                self.validate_statement(file, statement, &registry, &mut violations);
            }
        }

        violations
    }

    fn travers_statements_to_validate<'a>(
        &'a self,
        flatten_statements: &mut Vec<&'a Statement<'a>>,
        statement: &'a Statement<'a>,
    ) {
        // Only push top-level; we handle recursion ourselves to maintain class context.
        flatten_statements.push(statement);
    }
}

impl Rule {
    // -------------------------------------------------------------------------
    // Phase 1: Build TypeRegistry from all class/trait/interface declarations
    // -------------------------------------------------------------------------

    fn collect_types(&self, statement: &Statement<'_>, registry: &mut TypeRegistry) {
        match statement {
            Statement::Namespace(ns) => {
                for s in ns.statements().iter() {
                    self.collect_types(s, registry);
                }
            }
            Statement::Class(class) => {
                let name = class.name.value.to_string();
                let mut method_map = HashMap::new();
                let mut prop_map = HashMap::new();
                for member in class.members.iter() {
                    match member {
                        ClassLikeMember::Method(m) => {
                            if let Some(hint) = &m.return_type_hint {
                                if let Some(t) = self.extract_type_hint(&hint.hint) {
                                    method_map.insert(m.name.value.to_string(), t);
                                }
                            }
                        }
                        ClassLikeMember::Property(p) => {
                            if let Some(hint) = p.hint() {
                                if let Some(t) = self.extract_type_hint(hint) {
                                    for var in p.variables() {
                                        let prop_name =
                                            var.name.trim_start_matches('$').to_string();
                                        prop_map.insert(prop_name, t.clone());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                registry.methods.insert(name.clone(), method_map);
                registry.properties.insert(name, prop_map);
            }
            Statement::Trait(trait_def) => {
                let name = trait_def.name.value.to_string();
                let mut method_map = HashMap::new();
                let mut prop_map = HashMap::new();
                for member in trait_def.members.iter() {
                    match member {
                        ClassLikeMember::Method(m) => {
                            if let Some(hint) = &m.return_type_hint {
                                if let Some(t) = self.extract_type_hint(&hint.hint) {
                                    method_map.insert(m.name.value.to_string(), t);
                                }
                            }
                        }
                        ClassLikeMember::Property(p) => {
                            if let Some(hint) = p.hint() {
                                if let Some(t) = self.extract_type_hint(hint) {
                                    for var in p.variables() {
                                        let prop_name =
                                            var.name.trim_start_matches('$').to_string();
                                        prop_map.insert(prop_name, t.clone());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                registry.methods.insert(name.clone(), method_map);
                registry.properties.insert(name, prop_map);
            }
            Statement::Interface(iface) => {
                let name = iface.name.value.to_string();
                let mut method_map = HashMap::new();
                for member in iface.members.iter() {
                    if let ClassLikeMember::Method(m) = member {
                        if let Some(hint) = &m.return_type_hint {
                            if let Some(t) = self.extract_type_hint(&hint.hint) {
                                method_map.insert(m.name.value.to_string(), t);
                            }
                        }
                    }
                }
                registry.interfaces.insert(name.clone());
                registry.methods.insert(name, method_map);
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------------------
    // Phase 2: Build merged method map for a class (including trait methods)
    // -------------------------------------------------------------------------

    fn build_class_method_map(
        &self,
        class_name: &str,
        members: &Sequence<'_, ClassLikeMember<'_>>,
        registry: &TypeRegistry,
    ) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = HashMap::new();

        // First: include all direct method return types
        for member in members.iter() {
            match member {
                ClassLikeMember::Method(m) => {
                    if let Some(hint) = &m.return_type_hint {
                        if let Some(t) = self.extract_type_hint(&hint.hint) {
                            map.insert(m.name.value.to_string(), t);
                        }
                    }
                }
                ClassLikeMember::TraitUse(trait_use) => {
                    // Merge methods from used traits
                    for trait_name_id in trait_use.trait_names.iter() {
                        let trait_name = trait_name_id.value().to_string();
                        if let Some(trait_map) = registry.methods.get(&trait_name) {
                            for (method_name, ret_type) in trait_map {
                                // Don't override class's own method definitions
                                map.entry(method_name.clone())
                                    .or_insert_with(|| ret_type.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Normalize "self" in return types: if a method says it returns the class name itself,
        // treat it as "self" for the lookup later ONLY within the same class context.
        // Actually, it's better to keep it as "self" and contextualize it during resolution,
        // or just leave the class name.
        let class_owned = class_name.to_string();
        for val in map.values_mut() {
            if *val == "self" || *val == "static" {
                // Contextualize "self" to the actual class name to prevent false positives across classes
                *val = class_owned.clone();
            }
        }

        map
    }

    // -------------------------------------------------------------------------
    // Phase 3: Top-level statement validation dispatcher
    // -------------------------------------------------------------------------

    fn validate_statement(
        &self,
        file: &File<'_>,
        statement: &Statement<'_>,
        registry: &TypeRegistry,
        violations: &mut Vec<Violation>,
    ) {
        match statement {
            Statement::Class(class) => {
                let class_name = class.name.value.to_string();
                let method_map = self.build_class_method_map(&class_name, &class.members, registry);

                for member in class.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            let mut var_types = VarTypes::new();
                            self.track_parameters(&method.parameter_list, &mut var_types);
                            for stmt in block.statements.iter() {
                                self.check_statement(
                                    file,
                                    stmt,
                                    &method_map,
                                    &class_name,
                                    registry,
                                    &mut var_types,
                                    violations,
                                );
                            }
                        }
                    }
                }
            }
            Statement::Trait(trait_def) => {
                let trait_name = trait_def.name.value.to_string();
                let method_map =
                    self.build_class_method_map(&trait_name, &trait_def.members, registry);

                for member in trait_def.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            let mut var_types = VarTypes::new();
                            self.track_parameters(&method.parameter_list, &mut var_types);
                            for stmt in block.statements.iter() {
                                self.check_statement(
                                    file,
                                    stmt,
                                    &method_map,
                                    &trait_name,
                                    registry,
                                    &mut var_types,
                                    violations,
                                );
                            }
                        }
                    }
                }
            }
            Statement::Function(func) => {
                let mut var_types = VarTypes::new();
                self.track_parameters(&func.parameter_list, &mut var_types);
                for stmt in func.body.statements.iter() {
                    self.check_statement(
                        file,
                        stmt,
                        &HashMap::new(),
                        "",
                        registry,
                        &mut var_types,
                        violations,
                    );
                }
            }
            _ => {
                let mut var_types = VarTypes::new();
                self.check_statement(
                    file,
                    statement,
                    &HashMap::new(),
                    "",
                    registry,
                    &mut var_types,
                    violations,
                );
            }
        }
    }

    // -------------------------------------------------------------------------
    // Phase 4: Statement checker (with variable type tracking)
    // -------------------------------------------------------------------------

    fn check_statement(
        &self,
        file: &File<'_>,
        statement: &Statement<'_>,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &mut VarTypes,
        violations: &mut Vec<Violation>,
    ) {
        match statement {
            Statement::Expression(expr_stmt) => {
                if let Expression::Assignment(assign) = expr_stmt.expression {
                    let rhs_type = self.check_expression(
                        file,
                        assign.rhs,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                    if let Expression::Variable(Variable::Direct(d)) = assign.lhs {
                        let var_name = d.name.to_string();
                        if let Some(t) = rhs_type {
                            var_types.insert(var_name, t);
                        } else {
                            var_types.remove(&var_name);
                        }
                    }
                    self.check_expression(
                        file,
                        assign.lhs,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                } else {
                    self.check_expression(
                        file,
                        expr_stmt.expression,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                }
            }
            Statement::Return(ret_stmt) => {
                if let Some(expr) = ret_stmt.value {
                    self.check_expression(
                        file,
                        expr,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                }
            }
            Statement::Echo(echo_stmt) => {
                for expr in echo_stmt.values.iter() {
                    self.check_expression(
                        file,
                        expr,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                }
            }
            Statement::If(if_stmt) => {
                self.check_expression(
                    file,
                    if_stmt.condition,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                match &if_stmt.body {
                    IfBody::Statement(body) => {
                        self.check_statement(
                            file,
                            &body.statement,
                            method_map,
                            current_class,
                            registry,
                            var_types,
                            violations,
                        );
                        for clause in body.else_if_clauses.iter() {
                            self.check_expression(
                                file,
                                clause.condition,
                                method_map,
                                current_class,
                                registry,
                                var_types,
                                violations,
                            );
                            self.check_statement(
                                file,
                                &clause.statement,
                                method_map,
                                current_class,
                                registry,
                                var_types,
                                violations,
                            );
                        }
                        if let Some(else_clause) = &body.else_clause {
                            self.check_statement(
                                file,
                                &else_clause.statement,
                                method_map,
                                current_class,
                                registry,
                                var_types,
                                violations,
                            );
                        }
                    }
                    IfBody::ColonDelimited(body) => {
                        for s in body.statements.iter() {
                            self.check_statement(
                                file,
                                s,
                                method_map,
                                current_class,
                                registry,
                                var_types,
                                violations,
                            );
                        }
                        for clause in body.else_if_clauses.iter() {
                            self.check_expression(
                                file,
                                clause.condition,
                                method_map,
                                current_class,
                                registry,
                                var_types,
                                violations,
                            );
                            for s in clause.statements.iter() {
                                self.check_statement(
                                    file,
                                    s,
                                    method_map,
                                    current_class,
                                    registry,
                                    var_types,
                                    violations,
                                );
                            }
                        }
                        if let Some(else_clause) = &body.else_clause {
                            for s in else_clause.statements.iter() {
                                self.check_statement(
                                    file,
                                    s,
                                    method_map,
                                    current_class,
                                    registry,
                                    var_types,
                                    violations,
                                );
                            }
                        }
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.check_expression(
                    file,
                    while_stmt.condition,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                match &while_stmt.body {
                    WhileBody::Statement(body) => {
                        self.check_statement(
                            file,
                            body,
                            method_map,
                            current_class,
                            registry,
                            var_types,
                            violations,
                        );
                    }
                    WhileBody::ColonDelimited(body) => {
                        for s in body.statements.iter() {
                            self.check_statement(
                                file,
                                s,
                                method_map,
                                current_class,
                                registry,
                                var_types,
                                violations,
                            );
                        }
                    }
                }
            }
            Statement::DoWhile(do_while) => {
                self.check_expression(
                    file,
                    do_while.condition,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                self.check_statement(
                    file,
                    &do_while.statement,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
            }
            Statement::Switch(switch) => {
                self.check_expression(
                    file,
                    switch.expression,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                let cases = match &switch.body {
                    SwitchBody::BraceDelimited(body) => &body.cases,
                    SwitchBody::ColonDelimited(body) => &body.cases,
                };
                for case in cases.iter() {
                    match case {
                        SwitchCase::Expression(c) => {
                            self.check_expression(
                                file,
                                c.expression,
                                method_map,
                                current_class,
                                registry,
                                var_types,
                                violations,
                            );
                            for s in c.statements.iter() {
                                self.check_statement(
                                    file,
                                    s,
                                    method_map,
                                    current_class,
                                    registry,
                                    var_types,
                                    violations,
                                );
                            }
                        }
                        SwitchCase::Default(c) => {
                            for s in c.statements.iter() {
                                self.check_statement(
                                    file,
                                    s,
                                    method_map,
                                    current_class,
                                    registry,
                                    var_types,
                                    violations,
                                );
                            }
                        }
                    }
                }
            }
            Statement::Foreach(foreach) => {
                self.check_expression(
                    file,
                    foreach.expression,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                match &foreach.body {
                    ForeachBody::Statement(body) => {
                        self.check_statement(
                            file,
                            body,
                            method_map,
                            current_class,
                            registry,
                            var_types,
                            violations,
                        );
                    }
                    ForeachBody::ColonDelimited(body) => {
                        for s in body.statements.iter() {
                            self.check_statement(
                                file,
                                s,
                                method_map,
                                current_class,
                                registry,
                                var_types,
                                violations,
                            );
                        }
                    }
                }
            }
            Statement::Block(block) => {
                for s in block.statements.iter() {
                    self.check_statement(
                        file,
                        s,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                }
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------------------
    // Phase 5: Expression checker — core LoD detection logic
    //
    // Returns Some(type_name) if this expression has a known/resolved type,
    // None if the type is unknown (which triggers violation on any chaining).
    // -------------------------------------------------------------------------

    fn check_expression(
        &self,
        file: &File<'_>,
        expression: &Expression<'_>,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) -> Option<String> {
        match expression {
            // ------------------------------------------------------------------
            // Binary expressions: check both sides
            // ------------------------------------------------------------------
            Expression::Binary(binary) => {
                self.check_expression(
                    file,
                    binary.lhs,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                self.check_expression(
                    file,
                    binary.rhs,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None
            }

            // ------------------------------------------------------------------
            // Variable: resolve $this → "self", other vars via var_types
            // ------------------------------------------------------------------
            Expression::Variable(v) => {
                if let Variable::Direct(d) = v {
                    if d.name == "$this" {
                        return Some("self".to_string());
                    }
                    return var_types.get(d.name).cloned();
                }
                None
            }

            Expression::Instantiation(new_expr) => {
                let class_name = self.identifier_type_name(new_expr.class);
                if let Some(arg_list) = &new_expr.argument_list {
                    for arg in arg_list.arguments.iter() {
                        self.check_argument_expr(
                            file,
                            arg,
                            method_map,
                            current_class,
                            registry,
                            var_types,
                            violations,
                        );
                    }
                }
                class_name
            }

            // ------------------------------------------------------------------
            // Call expressions: method, null-safe method, static method, function
            // ------------------------------------------------------------------
            Expression::Call(call) => self.check_call(
                file,
                call,
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            ),

            // ------------------------------------------------------------------
            // Property / NullSafe property access: $obj->prop or $obj?->prop
            // This is a LoD violation if $obj is not self/current_class,
            // because you're reaching into a foreign object's internals.
            // The result type is always unknown (we don't track property types).
            // ------------------------------------------------------------------
            Expression::Access(access) => self.check_access(
                file,
                access,
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            ),

            // ------------------------------------------------------------------
            // Ternary / conditional: check all branches
            // ------------------------------------------------------------------
            Expression::Conditional(cond) => {
                self.check_expression(
                    file,
                    cond.condition,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                if let Some(then_expr) = cond.then {
                    self.check_expression(
                        file,
                        then_expr,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                }
                self.check_expression(
                    file,
                    cond.r#else,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None
            }

            // ------------------------------------------------------------------
            // Parenthesised: unwrap and recurse
            // ------------------------------------------------------------------
            Expression::Parenthesized(p) => self.check_expression(
                file,
                p.expression,
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            ),

            // ------------------------------------------------------------------
            // Unary prefix/postfix: recurse into operand
            // ------------------------------------------------------------------
            Expression::UnaryPrefix(u) => {
                self.check_expression(
                    file,
                    u.operand,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None
            }
            Expression::UnaryPostfix(u) => {
                self.check_expression(
                    file,
                    u.operand,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None
            }

            _ => None,
        }
    }

    // -------------------------------------------------------------------------
    // Helper: check Call variants
    // -------------------------------------------------------------------------

    fn check_call(
        &self,
        file: &File<'_>,
        call: &Call<'_>,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) -> Option<String> {
        match call {
            Call::Method(mc) => self.check_method_call_generic(
                file,
                mc.object,
                &mc.method,
                &mc.argument_list,
                mc.span(),
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            ),
            Call::NullSafeMethod(mc) => self.check_method_call_generic(
                file,
                mc.object,
                &mc.method,
                &mc.argument_list,
                mc.span(),
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            ),
            Call::StaticMethod(mc) => {
                // $class::method() — the class expression may be a class name or variable
                let class_type = self.check_expression(
                    file,
                    mc.class,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                // Check arguments
                for arg in mc.argument_list.arguments.iter() {
                    self.check_argument_expr(
                        file,
                        arg,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                }
                let method_name = self.member_selector_name(&mc.method);
                self.resolve_method_return(
                    class_type.as_deref(),
                    &method_name,
                    current_class,
                    method_map,
                    registry,
                )
            }
            Call::Function(fc) => {
                for arg in fc.argument_list.arguments.iter() {
                    self.check_argument_expr(
                        file,
                        arg,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                }
                None
            }
        }
    }

    /// Core logic for both `->method()` and `?->method()`.
    #[allow(clippy::too_many_arguments)]
    fn check_method_call_generic(
        &self,
        file: &File<'_>,
        object: &Expression<'_>,
        method_selector: &ClassLikeMemberSelector<'_>,
        argument_list: &ArgumentList<'_>,
        span: Span,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) -> Option<String> {
        // Resolve object type
        let object_type = self.resolve_object_type(
            file,
            object,
            method_map,
            current_class,
            registry,
            var_types,
            violations,
        );

        // Check arguments regardless of violation status
        for arg in argument_list.arguments.iter() {
            self.check_argument_expr(
                file,
                arg,
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            );
        }

        let method_name = self.member_selector_name(method_selector);

        // Determine if this is a method chain (`$a->b()->c()`) or a valid first call (`$a->b()`, `$this->foo->b()`)
        let is_chain = match object {
            Expression::Call(_) => true,
            Expression::Parenthesized(p) => matches!(p.expression, Expression::Call(_)),
            _ => false,
        };

        if !is_chain {
            // First method call on a base object (property, variable, new, etc.) is allowed.
            // We just resolve its return type for any subsequent chains.
            return self.resolve_method_return(
                object_type.as_deref(),
                &method_name,
                current_class,
                method_map,
                registry,
            );
        }

        // It is a chain. We allow chaining if it results in the current class,
        // or an interface, or if it's a fluent call on a foreign object.
        let is_fluent_on_foreign = match object {
            Expression::Call(call) => {
                let prev_object = match call {
                    Call::Method(mc) => Some(mc.object),
                    Call::NullSafeMethod(mc) => Some(mc.object),
                    _ => None,
                };
                if let Some(prev_obj) = prev_object {
                    let prev_type = self.resolve_object_type(
                        file,
                        prev_obj,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        &mut Vec::new(),
                    );
                    prev_type == object_type
                } else {
                    false
                }
            }
            _ => false,
        };

        match &object_type {
            Some(t) if self.is_own_type(t, current_class) || is_fluent_on_foreign => self
                .resolve_method_return(Some(t), &method_name, current_class, method_map, registry),
            Some(t) if registry.interfaces.contains(t) => self.resolve_method_return(
                Some(t),
                &method_name,
                current_class,
                method_map,
                registry,
            ),
            Some(t) => {
                // Foreign type — chaining is a violation
                let message = format!(
                    "Law of Demeter violation. Method '{}' is called on '{}', which is a foreign object.",
                    method_name, t
                );
                violations.push(self.new_violation(file, message, span));
                None
            }
            None => {
                // Unknown type — conservative: violation
                let message = format!(
                    "Law of Demeter violation. Method '{}' is called on an object of unknown type (possible foreign object).",
                    method_name
                );
                violations.push(self.new_violation(file, message, span));
                None
            }
        }
    }

    // -------------------------------------------------------------------------
    // Helper: resolve the type of the receiver object
    // -------------------------------------------------------------------------

    fn resolve_object_type(
        &self,
        file: &File<'_>,
        object: &Expression<'_>,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) -> Option<String> {
        match object {
            // $this → "self"
            Expression::Variable(Variable::Direct(d)) => {
                if d.name == "$this" {
                    Some("self".to_string())
                } else {
                    var_types.get(d.name).cloned()
                }
            }

            // new Foo() → "Foo"
            Expression::Instantiation(new_expr) => {
                if let Some(arg_list) = &new_expr.argument_list {
                    for arg in arg_list.arguments.iter() {
                        self.check_argument_expr(
                            file,
                            arg,
                            method_map,
                            current_class,
                            registry,
                            var_types,
                            violations,
                        );
                    }
                }
                self.identifier_type_name(new_expr.class)
            }

            // Another method call (chained) → recurse via check_expression
            Expression::Call(call) => self.check_call(
                file,
                call,
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            ),

            // Property access like $this->foo → the type of $this->foo is unknown
            // (we don't track property types), so any chaining on it is a violation.
            // But simply accessing $this->foo is NOT itself a violation.
            Expression::Access(access) => match access {
                Access::Property(pa) => {
                    // $obj->prop: first validate the object itself
                    let obj_type = self.resolve_object_type(
                        file,
                        pa.object,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                    if let Some(t) = &obj_type {
                        if self.is_own_type(t, current_class) {
                            let lookup_type = if t == "self" || t == "static" {
                                current_class
                            } else {
                                t
                            };
                            if let Some(mut prop_type) =
                                registry.properties.get(lookup_type).and_then(|props| {
                                    let prop_name = self.member_selector_name(&pa.property);
                                    props.get(prop_name.trim_start_matches('$')).cloned()
                                })
                            {
                                if prop_type == current_class {
                                    prop_type = "self".to_string();
                                }
                                return Some(prop_type);
                            }
                            return None;
                        }
                    }
                    None
                }
                Access::NullSafeProperty(pa) => {
                    let obj_type = self.resolve_object_type(
                        file,
                        pa.object,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                    if let Some(t) = &obj_type {
                        if self.is_own_type(t, current_class) {
                            let lookup_type = if t == "self" || t == "static" {
                                current_class
                            } else {
                                t
                            };
                            if let Some(mut prop_type) =
                                registry.properties.get(lookup_type).and_then(|props| {
                                    let prop_name = self.member_selector_name(&pa.property);
                                    props.get(prop_name.trim_start_matches('$')).cloned()
                                })
                            {
                                if prop_type == current_class {
                                    prop_type = "self".to_string();
                                }
                                return Some(prop_type);
                            }
                            return None;
                        }
                    }
                    None
                }
                _ => self.check_expression(
                    file,
                    object,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                ),
            },

            Expression::Parenthesized(p) => self.resolve_object_type(
                file,
                p.expression,
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            ),

            Expression::Identifier(id) => {
                let name = id.value().to_string();
                match name.as_str() {
                    "self" | "static" => Some("self".to_string()),
                    _ => Some(name),
                }
            }

            _ => None,
        }
    }

    fn check_access(
        &self,
        file: &File<'_>,
        access: &Access<'_>,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) -> Option<String> {
        match access {
            Access::Property(pa) => {
                self.check_expression(
                    file,
                    pa.object,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None
            }
            Access::NullSafeProperty(pa) => {
                self.check_expression(
                    file,
                    pa.object,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None
            }
            Access::StaticProperty(pa) => {
                self.check_expression(
                    file,
                    pa.class,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None
            }
            Access::ClassConstant(pa) => {
                self.check_expression(
                    file,
                    pa.class,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None
            }
        }
    }

    fn resolve_method_return(
        &self,
        object_type: Option<&str>,
        method_name: &str,
        current_class: &str,
        method_map: &HashMap<String, String>,
        registry: &TypeRegistry,
    ) -> Option<String> {
        if let Some(mut t) = object_type {
            if t == "self" || t == "static" {
                t = current_class;
            }
            if self.is_own_type(t, current_class) {
                if let Some(ret) = method_map.get(method_name) {
                    return Some(ret.clone());
                }
                if let Some(type_map) = registry.methods.get(t) {
                    if let Some(ret) = type_map.get(method_name) {
                        return Some(ret.clone());
                    }
                }
            } else if let Some(type_map) = registry.methods.get(t) {
                if let Some(ret) = type_map.get(method_name) {
                    return Some(ret.clone());
                }
            }
        }
        None
    }

    fn track_parameters(
        &self,
        parameters: &FunctionLikeParameterList<'_>,
        var_types: &mut VarTypes,
    ) {
        for param in parameters.parameters.iter() {
            if let Some(hint) = &param.hint {
                if let Some(t) = self.extract_type_hint(hint) {
                    var_types.insert(param.variable.name.to_string(), t);
                }
            }
        }
    }

    fn is_own_type(&self, t: &str, current_class: &str) -> bool {
        t == "self" || t == "static" || (!current_class.is_empty() && t == current_class)
    }

    fn member_selector_name(&self, selector: &ClassLikeMemberSelector<'_>) -> String {
        match selector {
            ClassLikeMemberSelector::Identifier(local_id) => local_id.value.to_string(),
            ClassLikeMemberSelector::Variable(v) => {
                if let Variable::Direct(d) = v {
                    d.name.to_string()
                } else {
                    "<dynamic>".to_string()
                }
            }
            ClassLikeMemberSelector::Expression(_) | ClassLikeMemberSelector::Missing(_) => {
                "<dynamic>".to_string()
            }
        }
    }

    fn identifier_type_name(&self, expr: &Expression<'_>) -> Option<String> {
        if let Expression::Identifier(id) = expr {
            Some(id.value().to_string())
        } else {
            None
        }
    }

    fn check_argument_expr(
        &self,
        file: &File<'_>,
        arg: &Argument<'_>,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) {
        match arg {
            Argument::Positional(a) => {
                self.check_expression(
                    file,
                    a.value,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
            }
            Argument::Named(a) => {
                self.check_expression(
                    file,
                    a.value,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
            }
        }
    }

    fn extract_type_hint(&self, hint: &Hint<'_>) -> Option<String> {
        match hint {
            Hint::Identifier(id) => Some(id.value().to_string()),
            Hint::Self_(_) => Some("self".to_string()),
            Hint::Static(_) => Some("static".to_string()),
            Hint::Parent(_) => Some("parent".to_string()),
            Hint::Nullable(n) => self.extract_type_hint(n.hint),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::rules::tests::analyze_file_for_rule;

    #[test]
    fn valid_fluent_interface() {
        let violations = analyze_file_for_rule("e14/valid.php", CODE);
        assert!(
            violations.is_empty(),
            "Expected no violations for valid fluent interface, got: {:?}",
            violations
        );
    }

    #[test]
    fn invalid_chaining_on_different_type() {
        let violations = analyze_file_for_rule("e14/invalid.php", CODE);
        assert!(
            !violations.is_empty(),
            "Expected violations for LoD-violating chaining, got none"
        );
    }

    #[test]
    fn invalid_cross_class_chaining() {
        let violations = analyze_file_for_rule("e14/invalid_cross_class.php", CODE);
        assert!(
            !violations.is_empty(),
            "Expected violations for cross-class LoD-violating chaining, got none"
        );
    }

    #[test]
    fn valid_trait_fluent_interface() {
        let violations = analyze_file_for_rule("e14/valid_trait_fluent.php", CODE);
        assert!(
            violations.is_empty(),
            "Expected no violations for trait fluent interface, got: {:?}",
            violations
        );
    }

    #[test]
    fn invalid_null_safe_chain() {
        let violations = analyze_file_for_rule("e14/invalid_null_safe_chain.php", CODE);
        assert!(
            !violations.is_empty(),
            "Expected violations for null-safe LoD-violating chaining, got none"
        );
    }

    #[test]
    fn invalid_property_chain() {
        let violations = analyze_file_for_rule("e14/invalid_property_chain.php", CODE);
        assert!(
            !violations.is_empty(),
            "Expected violations for property access LoD chaining, got none"
        );
    }

    #[test]
    fn valid_property_typed() {
        let violations = analyze_file_for_rule("e14/valid_property_typed.php", CODE);
        assert!(
            violations.is_empty(),
            "Expected no violations for property type chaining, got: {:?}",
            violations
        );
    }

    #[test]
    fn valid_interface_chain() {
        let violations = analyze_file_for_rule("e14/valid_interface_chain.php", CODE);
        assert!(
            violations.is_empty(),
            "Expected no violations for interface chaining, got: {:?}",
            violations
        );
    }

    #[test]
    fn valid_cross_file() {
        let rule = Rule::default();
        let arena = Bump::new();

        let path1 = std::path::PathBuf::from("./src/rules/examples/e14/dependency.php");
        let content1 =
            "<?php class DB { public function query(): DB { return $this; } }".to_string();
        let file1 = File::new(&arena, path1, content1);

        let path2 = std::path::PathBuf::from("./src/rules/examples/e14/usage.php");
        let content2 = "<?php class App { public function run(DB $db) { $db->query()->query(); } }"
            .to_string();
        let file2 = File::new(&arena, path2, content2);

        crate::rules::Rule::index_file(&rule, &file1);
        crate::rules::Rule::index_file(&rule, &file2);

        let violations = crate::rules::Rule::validate(
            &rule,
            &file2,
            file2.ast.unwrap().statements.first().unwrap(),
        );
        assert!(
            violations.is_empty(),
            "Expected no violations for cross-file injection, got: {:?}",
            violations
        );
    }
}
