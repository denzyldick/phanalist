use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;

pub(crate) fn function_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(
            String::from_utf8_lossy(identifier.value())
                .trim_start_matches('\\')
                .to_ascii_lowercase(),
        ),
        Expression::Parenthesized(parenthesized) => function_name(parenthesized.expression),
        _ => None,
    }
}

pub(crate) fn method_name(selector: &ClassLikeMemberSelector<'_>) -> Option<String> {
    match selector {
        ClassLikeMemberSelector::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.value).to_ascii_lowercase())
        }
        _ => None,
    }
}

/// Normalized name of any call form. Method and static calls return only the
/// method name, matching the taint sink registry.
pub(crate) fn call_name(call: &Call<'_>) -> Option<String> {
    match call {
        Call::Function(call) => function_name(call.function),
        Call::Method(call) => method_name(&call.method),
        Call::NullSafeMethod(call) => method_name(&call.method),
        Call::StaticMethod(call) => method_name(&call.method),
    }
}

/// Span covering the whole call in any form.
pub(crate) fn call_span(call: &Call<'_>) -> Span {
    match call {
        Call::Function(call) => call.span(),
        Call::Method(call) => call.span(),
        Call::NullSafeMethod(call) => call.span(),
        Call::StaticMethod(call) => call.span(),
    }
}

/// True for `$object->method()`, `$object?->method()` and `Class::method()`.
pub(crate) fn is_method_call(call: &Call<'_>) -> bool {
    !matches!(call, Call::Function(_))
}

pub(crate) fn direct_variable_name<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    let Expression::Variable(Variable::Direct(variable)) = expression else {
        return None;
    };
    std::str::from_utf8(variable.name)
        .ok()
        .map(|name| name.strip_prefix('$').unwrap_or(name))
}

pub(crate) fn literal_i64(expression: &Expression<'_>) -> Option<i64> {
    match expression {
        Expression::Literal(Literal::Integer(integer)) => {
            integer.value.and_then(|value| i64::try_from(value).ok())
        }
        Expression::UnaryPrefix(unary) => {
            let value = literal_i64(unary.operand)?;
            match unary.operator {
                UnaryPrefixOperator::Negation(_) => value.checked_neg(),
                UnaryPrefixOperator::Plus(_) => Some(value),
                _ => None,
            }
        }
        Expression::Parenthesized(parenthesized) => literal_i64(parenthesized.expression),
        _ => None,
    }
}

pub(crate) fn literal_string(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Literal(Literal::String(string)) => {
            Some(String::from_utf8_lossy(string.value?).to_string())
        }
        Expression::Parenthesized(parenthesized) => literal_string(parenthesized.expression),
        _ => None,
    }
}

pub(crate) fn literal_bool(expression: &Expression<'_>) -> Option<bool> {
    match expression.unparenthesized() {
        Expression::Literal(Literal::True(_)) => Some(true),
        Expression::Literal(Literal::False(_)) => Some(false),
        _ => None,
    }
}

/// Whether a literal array sets `key` to the literal boolean `true`.
///
/// Returns `None` when the expression is not a literal array, so callers can
/// fall back to positional arguments.
pub(crate) fn literal_array_flag(expression: &Expression<'_>, key: &str) -> Option<bool> {
    let elements = match expression.unparenthesized() {
        Expression::Array(array) => &array.elements,
        Expression::LegacyArray(array) => &array.elements,
        _ => return None,
    };

    for element in elements.iter() {
        if let ArrayElement::KeyValue(pair) = element {
            if literal_string(pair.key).as_deref() == Some(key) {
                return Some(literal_bool(pair.value) == Some(true));
            }
        }
    }

    Some(false)
}

pub(crate) fn literal_array_len(expression: &Expression<'_>) -> Option<usize> {
    let elements = match expression {
        Expression::Array(array) => &array.elements,
        Expression::LegacyArray(array) => &array.elements,
        Expression::Parenthesized(parenthesized) => {
            return literal_array_len(parenthesized.expression);
        }
        _ => return None,
    };

    if elements.iter().any(|element| {
        matches!(
            element,
            ArrayElement::Missing(_) | ArrayElement::Variadic(_)
        )
    }) {
        return None;
    }

    Some(elements.iter().count())
}

pub(crate) fn call_arguments<'a>(call: &'a Call<'a>) -> Vec<&'a Expression<'a>> {
    call.get_argument_list()
        .arguments
        .iter()
        .map(|argument| match argument {
            Argument::Positional(argument) => argument.value,
            Argument::Named(argument) => argument.value,
        })
        .collect()
}

pub(crate) fn walk_expression<'a>(
    expression: &'a Expression<'a>,
    visit: &mut impl FnMut(&'a Expression<'a>),
) {
    // Iterative pre-order traversal. Recursing here would exhaust the thread
    // stack on deeply nested expressions (for example a concatenation chain
    // with tens of thousands of terms in a generated fixture).
    let mut stack: Vec<&'a Expression<'a>> = vec![expression];
    while let Some(expression) = stack.pop() {
        visit(expression);

        if let Expression::Closure(closure) = expression {
            // Statement nesting is shallow in practice; expression nesting
            // inside the body is still walked iteratively.
            walk_statements(closure.body.statements.as_slice(), visit);
            continue;
        }

        push_expression_children(expression, &mut stack);
    }
}

fn push_expression_children<'a>(
    expression: &'a Expression<'a>,
    stack: &mut Vec<&'a Expression<'a>>,
) {
    let mut children: Vec<&'a Expression<'a>> = Vec::new();

    match expression {
        Expression::Binary(binary) => {
            children.push(binary.lhs);
            children.push(binary.rhs);
        }
        Expression::UnaryPrefix(unary) => children.push(unary.operand),
        Expression::UnaryPostfix(unary) => children.push(unary.operand),
        Expression::Parenthesized(parenthesized) => children.push(parenthesized.expression),
        Expression::CompositeString(string) => {
            for part in string.parts() {
                match part {
                    StringPart::Expression(expression) => children.push(expression),
                    StringPart::BracedExpression(braced) => children.push(braced.expression),
                    StringPart::Literal(_) => {}
                }
            }
        }
        Expression::Assignment(assignment) => {
            children.push(assignment.lhs);
            children.push(assignment.rhs);
        }
        Expression::Conditional(conditional) => {
            children.push(conditional.condition);
            if let Some(then) = conditional.then {
                children.push(then);
            }
            children.push(conditional.r#else);
        }
        Expression::Array(array) => push_array_element_children(&array.elements, &mut children),
        Expression::LegacyArray(array) => {
            push_array_element_children(&array.elements, &mut children)
        }
        Expression::List(list) => push_array_element_children(&list.elements, &mut children),
        Expression::ArrayAccess(access) => {
            children.push(access.array);
            children.push(access.index);
        }
        Expression::ArrayAppend(append) => children.push(append.array),
        Expression::ArrowFunction(function) => children.push(function.expression),
        Expression::Match(r#match) => {
            children.push(r#match.expression);
            for arm in r#match.arms.iter() {
                if let MatchArm::Expression(arm) = arm {
                    for condition in arm.conditions.iter() {
                        children.push(condition);
                    }
                }
                children.push(arm.expression());
            }
        }
        Expression::Yield(yield_) => match yield_ {
            Yield::Value(value) => {
                if let Some(value) = value.value {
                    children.push(value);
                }
            }
            Yield::Pair(pair) => {
                children.push(pair.key);
                children.push(pair.value);
            }
            Yield::From(from) => children.push(from.iterator),
        },
        Expression::Construct(construct) => push_construct_children(construct, &mut children),
        Expression::Throw(throw) => children.push(throw.exception),
        Expression::Clone(clone) => children.push(clone.object),
        Expression::Call(call) => push_call_children(call, &mut children),
        Expression::Instantiation(instantiation) => {
            children.push(instantiation.class);
            if let Some(arguments) = &instantiation.argument_list {
                push_argument_children(&arguments.arguments, &mut children);
            }
        }
        Expression::Pipe(pipe) => {
            children.push(pipe.input);
            children.push(pipe.callable);
        }
        Expression::Access(access) => match access {
            Access::Property(access) => children.push(access.object),
            Access::NullSafeProperty(access) => children.push(access.object),
            Access::StaticProperty(access) => children.push(access.class),
            Access::ClassConstant(access) => children.push(access.class),
        },
        _ => {}
    }

    // Reverse so the stack pops children in source order.
    stack.extend(children.into_iter().rev());
}

fn walk_statements<'a>(
    statements: &'a [Statement<'a>],
    visit: &mut impl FnMut(&'a Expression<'a>),
) {
    for statement in statements {
        walk_statement_expressions(statement, visit);
    }
}

pub(crate) fn walk_statement_expressions<'a>(
    statement: &'a Statement<'a>,
    visit: &mut impl FnMut(&'a Expression<'a>),
) {
    match statement {
        Statement::Expression(statement) => walk_expression(statement.expression, visit),
        Statement::Return(statement) => {
            if let Some(value) = statement.value {
                walk_expression(value, visit);
            }
        }
        Statement::Echo(statement) => {
            for value in statement.values.iter() {
                walk_expression(value, visit);
            }
        }
        Statement::If(statement) => {
            walk_expression(statement.condition, visit);
            walk_if_body(&statement.body, visit);
        }
        Statement::While(statement) => {
            walk_expression(statement.condition, visit);
            walk_while_body(&statement.body, visit);
        }
        Statement::DoWhile(statement) => {
            walk_statement_expressions(statement.statement, visit);
            walk_expression(statement.condition, visit);
        }
        Statement::For(statement) => {
            for expression in statement.initializations.iter() {
                walk_expression(expression, visit);
            }
            for expression in statement.conditions.iter() {
                walk_expression(expression, visit);
            }
            for expression in statement.increments.iter() {
                walk_expression(expression, visit);
            }
            walk_for_body(&statement.body, visit);
        }
        Statement::Foreach(statement) => {
            walk_expression(statement.expression, visit);
            walk_expression(statement.target.value(), visit);
            if let Some(key) = statement.target.key() {
                walk_expression(key, visit);
            }
            walk_foreach_body(&statement.body, visit);
        }
        Statement::Switch(statement) => {
            walk_expression(statement.expression, visit);
            for case in statement.body.cases().iter() {
                if let Some(expression) = case.expression() {
                    walk_expression(expression, visit);
                }
                walk_statements(case.statements(), visit);
            }
        }
        Statement::Block(block) => walk_statements(block.statements.as_slice(), visit),
        Statement::Namespace(namespace) => {
            walk_statements(namespace.statements().as_slice(), visit)
        }
        Statement::Try(statement) => {
            walk_statements(statement.block.statements.as_slice(), visit);
            for catch in statement.catch_clauses.iter() {
                walk_statements(catch.block.statements.as_slice(), visit);
            }
            if let Some(finally) = &statement.finally_clause {
                walk_statements(finally.block.statements.as_slice(), visit);
            }
        }
        Statement::Function(function) => {
            walk_statements(function.body.statements.as_slice(), visit)
        }
        Statement::Class(class) => walk_members(class.members.as_slice(), visit),
        Statement::Interface(interface) => walk_members(interface.members.as_slice(), visit),
        Statement::Trait(trait_) => walk_members(trait_.members.as_slice(), visit),
        Statement::Enum(enum_) => walk_members(enum_.members.as_slice(), visit),
        _ => {}
    }
}

fn push_array_element_children<'a>(
    elements: &'a TokenSeparatedSequence<'a, ArrayElement<'a>>,
    children: &mut Vec<&'a Expression<'a>>,
) {
    for element in elements.iter() {
        match element {
            ArrayElement::KeyValue(element) => {
                children.push(element.key);
                children.push(element.value);
            }
            ArrayElement::Value(element) => children.push(element.value),
            ArrayElement::Variadic(element) => children.push(element.value),
            ArrayElement::Missing(_) => {}
        }
    }
}

fn push_argument_children<'a>(
    arguments: &'a TokenSeparatedSequence<'a, Argument<'a>>,
    children: &mut Vec<&'a Expression<'a>>,
) {
    for argument in arguments.iter() {
        match argument {
            Argument::Positional(argument) => children.push(argument.value),
            Argument::Named(argument) => children.push(argument.value),
        }
    }
}

fn push_call_children<'a>(call: &'a Call<'a>, children: &mut Vec<&'a Expression<'a>>) {
    match call {
        Call::Function(call) => {
            children.push(call.function);
            push_argument_children(&call.argument_list.arguments, children);
        }
        Call::Method(call) => {
            children.push(call.object);
            push_argument_children(&call.argument_list.arguments, children);
        }
        Call::NullSafeMethod(call) => {
            children.push(call.object);
            push_argument_children(&call.argument_list.arguments, children);
        }
        Call::StaticMethod(call) => {
            children.push(call.class);
            push_argument_children(&call.argument_list.arguments, children);
        }
    }
}

fn push_construct_children<'a>(
    construct: &'a Construct<'a>,
    children: &mut Vec<&'a Expression<'a>>,
) {
    match construct {
        Construct::Isset(construct) => children.extend(construct.values.iter()),
        Construct::Empty(construct) => children.push(construct.value),
        Construct::Eval(construct) => children.push(construct.value),
        Construct::Include(construct) => children.push(construct.value),
        Construct::IncludeOnce(construct) => children.push(construct.value),
        Construct::Require(construct) => children.push(construct.value),
        Construct::RequireOnce(construct) => children.push(construct.value),
        Construct::Print(construct) => children.push(construct.value),
        Construct::Exit(construct) => {
            if let Some(arguments) = &construct.arguments {
                push_argument_children(&arguments.arguments, children);
            }
        }
        Construct::Die(construct) => {
            if let Some(arguments) = &construct.arguments {
                push_argument_children(&arguments.arguments, children);
            }
        }
    }
}

fn walk_if_body<'a>(body: &'a IfBody<'a>, visit: &mut impl FnMut(&'a Expression<'a>)) {
    match body {
        IfBody::Statement(body) => {
            walk_statement_expressions(body.statement, visit);
            for clause in body.else_if_clauses.iter() {
                walk_expression(clause.condition, visit);
                walk_statement_expressions(clause.statement, visit);
            }
            if let Some(clause) = &body.else_clause {
                walk_statement_expressions(clause.statement, visit);
            }
        }
        IfBody::ColonDelimited(body) => {
            walk_statements(body.statements.as_slice(), visit);
            for clause in body.else_if_clauses.iter() {
                walk_expression(clause.condition, visit);
                walk_statements(clause.statements.as_slice(), visit);
            }
            if let Some(clause) = &body.else_clause {
                walk_statements(clause.statements.as_slice(), visit);
            }
        }
    }
}

fn walk_while_body<'a>(body: &'a WhileBody<'a>, visit: &mut impl FnMut(&'a Expression<'a>)) {
    match body {
        WhileBody::Statement(statement) => walk_statement_expressions(statement, visit),
        WhileBody::ColonDelimited(body) => walk_statements(body.statements.as_slice(), visit),
    }
}

fn walk_for_body<'a>(body: &'a ForBody<'a>, visit: &mut impl FnMut(&'a Expression<'a>)) {
    match body {
        ForBody::Statement(statement) => walk_statement_expressions(statement, visit),
        ForBody::ColonDelimited(body) => walk_statements(body.statements.as_slice(), visit),
    }
}

fn walk_foreach_body<'a>(body: &'a ForeachBody<'a>, visit: &mut impl FnMut(&'a Expression<'a>)) {
    match body {
        ForeachBody::Statement(statement) => walk_statement_expressions(statement, visit),
        ForeachBody::ColonDelimited(body) => walk_statements(body.statements.as_slice(), visit),
    }
}

fn walk_members<'a>(
    members: &'a [ClassLikeMember<'a>],
    visit: &mut impl FnMut(&'a Expression<'a>),
) {
    for member in members {
        if let ClassLikeMember::Method(method) = member {
            if let MethodBody::Concrete(block) = &method.body {
                walk_statements(block.statements.as_slice(), visit);
            }
        }
    }
}

pub(crate) fn walk_own_expressions<'a>(
    statement: &'a Statement<'a>,
    visit: &mut impl FnMut(&'a Expression<'a>),
) {
    match statement {
        Statement::Expression(statement) => walk_expression(statement.expression, visit),
        Statement::Return(statement) => {
            if let Some(value) = statement.value {
                walk_expression(value, visit);
            }
        }
        Statement::Echo(statement) => {
            for value in statement.values.iter() {
                walk_expression(value, visit);
            }
        }
        Statement::If(statement) => walk_expression(statement.condition, visit),
        Statement::While(statement) => walk_expression(statement.condition, visit),
        Statement::DoWhile(statement) => walk_expression(statement.condition, visit),
        Statement::For(statement) => {
            for expression in statement.initializations.iter() {
                walk_expression(expression, visit);
            }
            for expression in statement.conditions.iter() {
                walk_expression(expression, visit);
            }
            for expression in statement.increments.iter() {
                walk_expression(expression, visit);
            }
        }
        Statement::Foreach(statement) => {
            walk_expression(statement.expression, visit);
            walk_expression(statement.target.value(), visit);
            if let Some(key) = statement.target.key() {
                walk_expression(key, visit);
            }
        }
        Statement::Switch(statement) => {
            walk_expression(statement.expression, visit);
            for case in statement.body.cases().iter() {
                if let Some(expression) = case.expression() {
                    walk_expression(expression, visit);
                }
            }
        }
        Statement::Unset(statement) => {
            for value in statement.values.iter() {
                walk_expression(value, visit);
            }
        }
        _ => {}
    }
}
