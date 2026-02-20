use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;
use mago_ast::ast::access::Access;
use mago_ast::ast::class_like::member::{ClassLikeMember, ClassLikeMemberSelector};
use mago_ast::ast::control_flow::r#if::IfBody;
use mago_ast::ast::control_flow::switch::SwitchBody;
use mago_ast::ast::expression::Expression;
use mago_ast::ast::identifier::Identifier;
use mago_ast::ast::r#loop::foreach::ForeachBody;
use mago_ast::ast::r#loop::r#while::WhileBody;
use mago_ast::ast::type_hint::Hint;
use mago_ast::ast::Statement;
use mago_ast::Call;
use mago_ast::MethodBody;
use mago_ast::Variable;
use mago_span::HasSpan;
use std::collections::HashMap;

static CODE: &str = "E0014";
static DESCRIPTION: &str =
    "Law of Demeter violation. Method chaining should be avoided unless returning the same object type.";

/// Maps class/trait/interface name → (method_name → return_type_string)
type TypeRegistry = HashMap<String, HashMap<String, String>>;
/// Maps local variable name → resolved type string
type VarTypes = HashMap<String, String>;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File) -> bool {
        true
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        // Build the type registry from ALL statements in the file so that cross-file
        // references (e.g. a class using a trait defined elsewhere in the same file)
        // are resolvable. Fall back to building from just this statement if no AST.
        let mut registry = TypeRegistry::new();
        if let Some(program) = &file.ast {
            for s in program.statements.iter() {
                self.collect_types(file, s, &mut registry);
            }
        } else {
            // Fallback: only this statement
            match statement {
                Statement::Namespace(ns) => {
                    for s in ns.statements().iter() {
                        self.collect_types(file, s, &mut registry);
                    }
                }
                _ => {
                    self.collect_types(file, statement, &mut registry);
                }
            }
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
        flatten_statements: &mut Vec<&'a Statement>,
        statement: &'a Statement,
    ) {
        // Only push top-level; we handle recursion ourselves to maintain class context.
        flatten_statements.push(statement);
    }
}

impl Rule {
    // -------------------------------------------------------------------------
    // Phase 1: Build TypeRegistry from all class/trait/interface declarations
    // -------------------------------------------------------------------------

    fn collect_types(&self, file: &File, statement: &Statement, registry: &mut TypeRegistry) {
        match statement {
            Statement::Namespace(ns) => {
                for s in ns.statements().iter() {
                    self.collect_types(file, s, registry);
                }
            }
            Statement::Class(class) => {
                let name = file.interner.lookup(&class.name.value).to_string();
                let mut map = HashMap::new();
                for member in class.members.iter() {
                    if let ClassLikeMember::Method(m) = member {
                        let method_name = file.interner.lookup(&m.name.value).to_string();
                        if let Some(hint) = &m.return_type_hint {
                            if let Some(t) = self.extract_type_hint(file, &hint.hint) {
                                map.insert(method_name, t);
                            }
                        }
                    }
                }
                registry.insert(name, map);
            }
            Statement::Trait(trait_def) => {
                let name = file.interner.lookup(&trait_def.name.value).to_string();
                let mut map = HashMap::new();
                for member in trait_def.members.iter() {
                    if let ClassLikeMember::Method(m) = member {
                        let method_name = file.interner.lookup(&m.name.value).to_string();
                        if let Some(hint) = &m.return_type_hint {
                            if let Some(t) = self.extract_type_hint(file, &hint.hint) {
                                map.insert(method_name, t);
                            }
                        }
                    }
                }
                registry.insert(name, map);
            }
            Statement::Interface(iface) => {
                let name = file.interner.lookup(&iface.name.value).to_string();
                let mut map = HashMap::new();
                for member in iface.members.iter() {
                    if let ClassLikeMember::Method(m) = member {
                        let method_name = file.interner.lookup(&m.name.value).to_string();
                        if let Some(hint) = &m.return_type_hint {
                            if let Some(t) = self.extract_type_hint(file, &hint.hint) {
                                map.insert(method_name, t);
                            }
                        }
                    }
                }
                registry.insert(name, map);
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------------------
    // Phase 2: Build merged method map for a class (including trait methods)
    // -------------------------------------------------------------------------

    fn build_class_method_map(
        &self,
        file: &File,
        class_name: &str,
        members: &mago_ast::Sequence<ClassLikeMember>,
        registry: &TypeRegistry,
    ) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = HashMap::new();

        // First: include all direct method return types
        for member in members.iter() {
            match member {
                ClassLikeMember::Method(m) => {
                    let method_name = file.interner.lookup(&m.name.value).to_string();
                    if let Some(hint) = &m.return_type_hint {
                        if let Some(t) = self.extract_type_hint(file, &hint.hint) {
                            map.insert(method_name, t);
                        }
                    }
                }
                ClassLikeMember::TraitUse(trait_use) => {
                    // Merge methods from used traits
                    for trait_name_id in trait_use.trait_names.iter() {
                        let trait_name = self.lookup_identifier(file, trait_name_id);
                        if let Some(trait_map) = registry.get(&trait_name) {
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
        // treat it as "self" for the lookup later.
        // (This handles trait methods that say `return_type = ClassName` explicitly.)
        let class_owned = class_name.to_string();
        for val in map.values_mut() {
            if *val == class_owned {
                *val = "self".to_string();
            }
        }

        map
    }

    // -------------------------------------------------------------------------
    // Phase 3: Top-level statement validation dispatcher
    // -------------------------------------------------------------------------

    fn validate_statement(
        &self,
        file: &File,
        statement: &Statement,
        registry: &TypeRegistry,
        violations: &mut Vec<Violation>,
    ) {
        match statement {
            Statement::Class(class) => {
                let class_name = file.interner.lookup(&class.name.value).to_string();
                let method_map =
                    self.build_class_method_map(file, &class_name, &class.members, registry);

                for member in class.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            let mut var_types = VarTypes::new();
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
                let trait_name = file.interner.lookup(&trait_def.name.value).to_string();
                let method_map =
                    self.build_class_method_map(file, &trait_name, &trait_def.members, registry);

                for member in trait_def.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            let mut var_types = VarTypes::new();
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
            _ => {
                // For standalone functions or procedural code, check without class context
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
        file: &File,
        statement: &Statement,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &mut VarTypes,
        violations: &mut Vec<Violation>,
    ) {
        match statement {
            Statement::Expression(expr_stmt) => {
                // If this is an assignment, try to track variable type
                if let Expression::Assignment(assign) = expr_stmt.expression.as_ref() {
                    // Check RHS for violations first
                    let rhs_type = self.check_expression(
                        file,
                        &assign.rhs,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                    // Track variable type if LHS is a simple variable
                    if let Expression::Variable(Variable::Direct(d)) = assign.lhs.as_ref() {
                        let var_name = file.interner.lookup(&d.name).to_string();
                        if let Some(t) = rhs_type {
                            var_types.insert(var_name, t);
                        } else {
                            // Remove stale type info so chaining on it flags a violation
                            var_types.remove(&var_name);
                        }
                    }
                    // Also check LHS (in case of compound assignments on chains)
                    self.check_expression(
                        file,
                        &assign.lhs,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                } else {
                    self.check_expression(
                        file,
                        &expr_stmt.expression,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                }
            }
            Statement::Return(ret_stmt) => {
                if let Some(expr) = &ret_stmt.value {
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
                    &if_stmt.condition,
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
                                &clause.condition,
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
                                &clause.condition,
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
                    &while_stmt.condition,
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
                    &do_while.condition,
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
                    &switch.expression,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                let check_cases = |cases: &mago_ast::Sequence<_>,
                                   slf: &Rule,
                                   violations: &mut Vec<Violation>,
                                   var_types: &mut VarTypes| {
                    for case in cases.iter() {
                        match case {
                            mago_ast::ast::control_flow::switch::SwitchCase::Expression(c) => {
                                slf.check_expression(
                                    file,
                                    &c.expression,
                                    method_map,
                                    current_class,
                                    registry,
                                    var_types,
                                    violations,
                                );
                                for s in c.statements.iter() {
                                    slf.check_statement(
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
                            mago_ast::ast::control_flow::switch::SwitchCase::Default(c) => {
                                for s in c.statements.iter() {
                                    slf.check_statement(
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
                };
                match &switch.body {
                    SwitchBody::BraceDelimited(body) => {
                        check_cases(&body.cases, self, violations, var_types);
                    }
                    SwitchBody::ColonDelimited(body) => {
                        check_cases(&body.cases, self, violations, var_types);
                    }
                }
            }
            Statement::Foreach(foreach) => {
                self.check_expression(
                    file,
                    &foreach.expression,
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
        file: &File,
        expression: &Expression,
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
                    &binary.lhs,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                self.check_expression(
                    file,
                    &binary.rhs,
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
                    let name = file.interner.lookup(&d.name);
                    if name == "$this" {
                        return Some("self".to_string());
                    }
                    // Resolve from tracked variable types
                    return var_types.get(name).cloned();
                }
                None
            }

            // ------------------------------------------------------------------
            // Instantiation: new Foo() → "Foo"
            // ------------------------------------------------------------------
            Expression::Instantiation(new_expr) => {
                let class_name = self.identifier_type_name(file, &new_expr.class);
                // Also check constructor arguments (may be absent for `new Foo`)
                if let Some(arg_list) = &new_expr.arguments {
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
                    &cond.condition,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                if let Some(then_expr) = &cond.then {
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
                    &cond.r#else,
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
                &p.expression,
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
                    &u.operand,
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
                    &u.operand,
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
        file: &File,
        call: &Call,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) -> Option<String> {
        match call {
            Call::Method(mc) => self.check_method_call_generic(
                file,
                mc.object.as_ref(),
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
                mc.object.as_ref(),
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
                    &mc.class,
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
                // Resolve return type of the static method
                let method_name = self.member_selector_name(file, &mc.method);
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
                None // Function return type is unknown
            }
        }
    }

    /// Core logic for both `->method()` and `?->method()`.
    #[allow(clippy::too_many_arguments)]
    fn check_method_call_generic(
        &self,
        file: &File,
        object: &Expression,
        method_selector: &ClassLikeMemberSelector,
        argument_list: &mago_ast::ast::argument::ArgumentList,
        span: mago_span::Span,
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

        // Determine if chaining on this object is safe
        let method_name = self.member_selector_name(file, method_selector);

        match &object_type {
            Some(t) if self.is_own_type(t, current_class) => {
                // Safe: look up return type of this method
                self.resolve_method_return(
                    Some(t),
                    &method_name,
                    current_class,
                    method_map,
                    registry,
                )
            }
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
        file: &File,
        object: &Expression,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) -> Option<String> {
        match object {
            // $this → "self"
            Expression::Variable(Variable::Direct(d)) => {
                let name = file.interner.lookup(&d.name);
                if name == "$this" {
                    Some("self".to_string())
                } else {
                    // Look up variable type from tracking
                    var_types.get(name).cloned()
                }
            }

            // new Foo() → "Foo"
            Expression::Instantiation(new_expr) => {
                if let Some(arg_list) = &new_expr.arguments {
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
                self.identifier_type_name(file, &new_expr.class)
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
                        &pa.object,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                    if let Some(t) = &obj_type {
                        if self.is_own_type(t, current_class) {
                            // Accessing own property is fine; but its TYPE is unknown → None
                            return None;
                        }
                    }
                    // Accessing a property of a foreign object
                    // The caller will detect the violation when chaining on this None
                    None
                }
                Access::NullSafeProperty(pa) => {
                    let obj_type = self.resolve_object_type(
                        file,
                        &pa.object,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    );
                    if let Some(t) = &obj_type {
                        if self.is_own_type(t, current_class) {
                            return None;
                        }
                    }
                    None
                }
                _ => {
                    // StaticProperty / ClassConstant — recurse
                    self.check_expression(
                        file,
                        object,
                        method_map,
                        current_class,
                        registry,
                        var_types,
                        violations,
                    )
                }
            },

            // Parenthesised: unwrap
            Expression::Parenthesized(p) => self.resolve_object_type(
                file,
                &p.expression,
                method_map,
                current_class,
                registry,
                var_types,
                violations,
            ),

            // Static class name identifier used directly e.g. ClassName::staticMethod()
            Expression::Identifier(id) => {
                let name = match id {
                    Identifier::Local(l) => file.interner.lookup(&l.value).to_string(),
                    Identifier::Qualified(q) => file.interner.lookup(&q.value).to_string(),
                    Identifier::FullyQualified(f) => file.interner.lookup(&f.value).to_string(),
                };
                // "self", "static", "parent" keywords used as class names
                match name.as_str() {
                    "self" | "static" => Some("self".to_string()),
                    _ => Some(name),
                }
            }

            _ => None,
        }
    }

    // -------------------------------------------------------------------------
    // Helper: check Access expressions (property access used standalone)
    // -------------------------------------------------------------------------

    fn check_access(
        &self,
        file: &File,
        access: &Access,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) -> Option<String> {
        match access {
            Access::Property(pa) => {
                // Standalone $obj->prop — validate the object part for chaining in it
                self.check_expression(
                    file,
                    &pa.object,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
                None // Property type is always unknown
            }
            Access::NullSafeProperty(pa) => {
                self.check_expression(
                    file,
                    &pa.object,
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
                    &pa.class,
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
                    &pa.class,
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

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    /// Resolve a method's return type from the method map or registry.
    fn resolve_method_return(
        &self,
        object_type: Option<&str>,
        method_name: &str,
        current_class: &str,
        method_map: &HashMap<String, String>,
        registry: &TypeRegistry,
    ) -> Option<String> {
        if let Some(t) = object_type {
            if self.is_own_type(t, current_class) {
                // Look up in the current class's method map
                if let Some(ret) = method_map.get(method_name) {
                    return Some(ret.clone());
                }
            } else {
                // Look up in the type registry for cross-class resolution
                if let Some(type_map) = registry.get(t) {
                    if let Some(ret) = type_map.get(method_name) {
                        return Some(ret.clone());
                    }
                }
            }
        }
        None
    }

    /// Returns true if `t` represents the current class (self, static, or by name).
    fn is_own_type(&self, t: &str, current_class: &str) -> bool {
        t == "self" || t == "static" || (!current_class.is_empty() && t == current_class)
    }

    /// Extract string name from a `ClassLikeMemberSelector`.
    fn member_selector_name(&self, file: &File, selector: &ClassLikeMemberSelector) -> String {
        match selector {
            ClassLikeMemberSelector::Identifier(local_id) => {
                file.interner.lookup(&local_id.value).to_string()
            }
            ClassLikeMemberSelector::Variable(v) => {
                if let Variable::Direct(d) = v {
                    format!("${}", file.interner.lookup(&d.name))
                } else {
                    "<dynamic>".to_string()
                }
            }
            ClassLikeMemberSelector::Expression(_) => "<dynamic>".to_string(),
        }
    }

    /// Extract string type name from an expression used as a class/instantiation target.
    fn identifier_type_name(&self, file: &File, expr: &Expression) -> Option<String> {
        if let Expression::Identifier(id) = expr {
            Some(match id {
                Identifier::Local(l) => file.interner.lookup(&l.value).to_string(),
                Identifier::Qualified(q) => file.interner.lookup(&q.value).to_string(),
                Identifier::FullyQualified(f) => file.interner.lookup(&f.value).to_string(),
            })
        } else {
            None
        }
    }

    /// Lookup string for an Identifier node (used in trait names etc).
    fn lookup_identifier(&self, file: &File, id: &Identifier) -> String {
        match id {
            Identifier::Local(l) => file.interner.lookup(&l.value).to_string(),
            Identifier::Qualified(q) => file.interner.lookup(&q.value).to_string(),
            Identifier::FullyQualified(f) => file.interner.lookup(&f.value).to_string(),
        }
    }

    /// Check an argument expression for LoD violations (does not affect return type).
    fn check_argument_expr(
        &self,
        file: &File,
        arg: &mago_ast::ast::argument::Argument,
        method_map: &HashMap<String, String>,
        current_class: &str,
        registry: &TypeRegistry,
        var_types: &VarTypes,
        violations: &mut Vec<Violation>,
    ) {
        match arg {
            mago_ast::ast::argument::Argument::Positional(a) => {
                self.check_expression(
                    file,
                    &a.value,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
            }
            mago_ast::ast::argument::Argument::Named(a) => {
                self.check_expression(
                    file,
                    &a.value,
                    method_map,
                    current_class,
                    registry,
                    var_types,
                    violations,
                );
            }
        }
    }

    /// Extract a printable type string from a return type Hint.
    fn extract_type_hint(&self, file: &File, hint: &Hint) -> Option<String> {
        match hint {
            Hint::Identifier(id) => Some(match id {
                Identifier::Local(l) => file.interner.lookup(&l.value).to_string(),
                Identifier::Qualified(q) => file.interner.lookup(&q.value).to_string(),
                Identifier::FullyQualified(f) => file.interner.lookup(&f.value).to_string(),
            }),
            Hint::Self_(_) => Some("self".to_string()),
            Hint::Static(_) => Some("static".to_string()),
            Hint::Parent(_) => Some("parent".to_string()),
            // For nullable types like ?self, unwrap and recurse
            Hint::Nullable(n) => self.extract_type_hint(file, &n.hint),
            _ => None,
        }
    }

    // new_violation is inherited from the Rule trait in mod.rs — no override needed
}

#[cfg(test)]
mod tests {
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
}
