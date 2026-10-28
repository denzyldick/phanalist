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

pub(crate) fn direct_variable_name<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    let Expression::Variable(Variable::Direct(variable)) = expression else {
        return None;
    };
    std::str::from_utf8(variable.name).ok()
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
    visit(expression);

    match expression {
        Expression::Binary(binary) => {
            walk_expression(binary.lhs, visit);
            walk_expression(binary.rhs, visit);
        }
        Expression::UnaryPrefix(unary) => walk_expression(unary.operand, visit),
        Expression::UnaryPostfix(unary) => walk_expression(unary.operand, visit),
        Expression::Parenthesized(parenthesized) => {
            walk_expression(parenthesized.expression, visit);
        }
        Expression::CompositeString(string) => {
            for part in string.parts() {
                match part {
                    StringPart::Expression(expression) => walk_expression(expression, visit),
                    StringPart::BracedExpression(braced) => {
                        walk_expression(braced.expression, visit)
                    }
                    StringPart::Literal(_) => {}
                }
            }
        }
        Expression::Assignment(assignment) => {
            walk_expression(assignment.lhs, visit);
            walk_expression(assignment.rhs, visit);
        }
        Expression::Conditional(conditional) => {
            walk_expression(conditional.condition, visit);
            if let Some(then) = conditional.then {
                walk_expression(then, visit);
            }
            walk_expression(conditional.r#else, visit);
        }
        Expression::Array(array) => walk_array_elements(&array.elements, visit),
        Expression::LegacyArray(array) => walk_array_elements(&array.elements, visit),
        Expression::List(list) => walk_array_elements(&list.elements, visit),
        Expression::ArrayAccess(access) => {
            walk_expression(access.array, visit);
            walk_expression(access.index, visit);
        }
        Expression::ArrayAppend(append) => walk_expression(append.array, visit),
        Expression::Closure(closure) => walk_statements(closure.body.statements.as_slice(), visit),
        Expression::ArrowFunction(function) => walk_expression(function.expression, visit),
        Expression::Match(r#match) => {
            walk_expression(r#match.expression, visit);
            for arm in r#match.arms.iter() {
                if let MatchArm::Expression(arm) = arm {
                    for condition in arm.conditions.iter() {
                        walk_expression(condition, visit);
                    }
                }
                walk_expression(arm.expression(), visit);
            }
        }
        Expression::Yield(yield_) => match yield_ {
            Yield::Value(value) => {
                if let Some(value) = value.value {
                    walk_expression(value, visit);
                }
            }
            Yield::Pair(pair) => {
                walk_expression(pair.key, visit);
                walk_expression(pair.value, visit);
            }
            Yield::From(from) => walk_expression(from.iterator, visit),
        },
        Expression::Construct(construct) => walk_construct(construct, visit),
        Expression::Throw(throw) => walk_expression(throw.exception, visit),
        Expression::Clone(clone) => walk_expression(clone.object, visit),
        Expression::Call(call) => walk_call(call, visit),
        Expression::Instantiation(instantiation) => {
            walk_expression(instantiation.class, visit);
            if let Some(arguments) = &instantiation.argument_list {
                walk_arguments(&arguments.arguments, visit);
            }
        }
        Expression::Pipe(pipe) => {
            walk_expression(pipe.input, visit);
            walk_expression(pipe.callable, visit);
        }
        Expression::Access(access) => match access {
            Access::Property(access) => walk_expression(access.object, visit),
            Access::NullSafeProperty(access) => walk_expression(access.object, visit),
            Access::StaticProperty(access) => walk_expression(access.class, visit),
            Access::ClassConstant(access) => walk_expression(access.class, visit),
        },
        _ => {}
    }
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

fn walk_array_elements<'a>(
    elements: &'a TokenSeparatedSequence<'a, ArrayElement<'a>>,
    visit: &mut impl FnMut(&'a Expression<'a>),
) {
    for element in elements.iter() {
        match element {
            ArrayElement::KeyValue(element) => {
                walk_expression(element.key, visit);
                walk_expression(element.value, visit);
            }
            ArrayElement::Value(element) => walk_expression(element.value, visit),
            ArrayElement::Variadic(element) => walk_expression(element.value, visit),
            ArrayElement::Missing(_) => {}
        }
    }
}

fn walk_arguments<'a>(
    arguments: &'a TokenSeparatedSequence<'a, Argument<'a>>,
    visit: &mut impl FnMut(&'a Expression<'a>),
) {
    for argument in arguments.iter() {
        match argument {
            Argument::Positional(argument) => walk_expression(argument.value, visit),
            Argument::Named(argument) => walk_expression(argument.value, visit),
        }
    }
}

fn walk_call<'a>(call: &'a Call<'a>, visit: &mut impl FnMut(&'a Expression<'a>)) {
    match call {
        Call::Function(call) => {
            walk_expression(call.function, visit);
            walk_arguments(&call.argument_list.arguments, visit);
        }
        Call::Method(call) => {
            walk_expression(call.object, visit);
            walk_arguments(&call.argument_list.arguments, visit);
        }
        Call::NullSafeMethod(call) => {
            walk_expression(call.object, visit);
            walk_arguments(&call.argument_list.arguments, visit);
        }
        Call::StaticMethod(call) => {
            walk_expression(call.class, visit);
            walk_arguments(&call.argument_list.arguments, visit);
        }
    }
}

fn walk_construct<'a>(construct: &'a Construct<'a>, visit: &mut impl FnMut(&'a Expression<'a>)) {
    match construct {
        Construct::Isset(construct) => {
            for value in construct.values.iter() {
                walk_expression(value, visit);
            }
        }
        Construct::Empty(construct) => walk_expression(construct.value, visit),
        Construct::Eval(construct) => walk_expression(construct.value, visit),
        Construct::Include(construct) => walk_expression(construct.value, visit),
        Construct::IncludeOnce(construct) => walk_expression(construct.value, visit),
        Construct::Require(construct) => walk_expression(construct.value, visit),
        Construct::RequireOnce(construct) => walk_expression(construct.value, visit),
        Construct::Print(construct) => walk_expression(construct.value, visit),
        Construct::Exit(construct) => {
            if let Some(arguments) = &construct.arguments {
                walk_arguments(&arguments.arguments, visit);
            }
        }
        Construct::Die(construct) => {
            if let Some(arguments) = &construct.arguments {
                walk_arguments(&arguments.arguments, visit);
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
