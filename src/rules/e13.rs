use std::collections::{HashMap, HashSet};

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_name, is_method_call, method_name as selector_method_name, walk_statement_expressions,
};

pub(crate) static CODE: &str = "E0013";
static DESCRIPTION: &str = "Private method not being called.";

pub struct Rule {}

impl crate::rules::Rule for Rule {
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

        if let Statement::Class(class) = statement {
            // Method names in PHP are case-insensitive, so both maps are keyed
            // by the lowercased name while the original spelling is kept for
            // the diagnostic.
            let mut private_methods: HashMap<String, (String, Span)> = HashMap::new();
            let mut called_methods: HashSet<String> = HashSet::new();

            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    let method_name = String::from_utf8_lossy(method.name.value).into_owned();
                    let is_private = method
                        .modifiers
                        .iter()
                        .any(|m| matches!(m, Modifier::Private(_)));

                    if is_private {
                        private_methods.insert(
                            method_name.to_ascii_lowercase(),
                            (method_name, method.span()),
                        );
                    }

                    // Walk every expression in the body so calls nested in
                    // assignments, arguments, conditions, closures, etc. are
                    // all seen, not just expression statements and returns.
                    if let MethodBody::Concrete(block) = &method.body {
                        for stmt in block.statements.iter() {
                            walk_statement_expressions(stmt, &mut |expression| {
                                match expression {
                                    Expression::Call(call) if is_method_call(call) => {
                                        if let Some(name) = call_name(call) {
                                            called_methods.insert(name);
                                        }
                                    }
                                    // First-class callables such as
                                    // `$this->helper(...)` also use the method.
                                    Expression::PartialApplication(application) => {
                                        let selector = match application {
                                            PartialApplication::Method(method) => {
                                                Some(&method.method)
                                            }
                                            PartialApplication::StaticMethod(method) => {
                                                Some(&method.method)
                                            }
                                            PartialApplication::Function(_) => None,
                                        };
                                        if let Some(name) = selector
                                            .and_then(|selector| selector_method_name(selector))
                                        {
                                            called_methods.insert(name);
                                        }
                                    }
                                    _ => {}
                                }
                            });
                        }
                    }
                }
            }

            // Report private methods that are never called
            for (name, (original_name, span)) in &private_methods {
                if !called_methods.contains(name) {
                    let message = Message::new(
                        "E0013:private-method-not-called",
                        "The private method {name} is not being called. ",
                    )
                    .arg("name", original_name.clone());
                    violations.push(self.new_violation(file, message, *span));
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::{analyze_file_for_rule, analyze_source_for_rule};

    use super::*;

    fn reported_methods(source: &str) -> Vec<String> {
        analyze_source_for_rule(source, "e13-inline.php", CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e13/private_method_not_being_called.php", CODE);
        println!("{}", violations.len());
        assert!(violations.len().eq(&3));
    }

    #[test]
    fn reported_issue_snippet_produces_no_violation() {
        let source = r#"<?php
final class EntityWithFileInfo
{
    public function getFileInfo(): object
    {
        return new stdClass();
    }
}

final class Assert
{
    public static function string(mixed $value): void
    {
    }
}

final class PathGenerator
{
    public function generateEntityPath(EntityWithFileInfo $entity, ?string $filename = null): string
    {
        $filename ??= $this->getBasenameOfFilePath($entity);

        return sprintf('%s/%s', $this->getRootPathForEntity($entity), $filename);
    }

    private function getBasenameOfFilePath(EntityWithFileInfo $entity): string
    {
        $path = $entity->getFileInfo()->getPath();
        Assert::string($path);

        return basename($path);
    }

    private function getRootPathForEntity(EntityWithFileInfo $entity): string
    {
        return '/root';
    }
}
"#;

        assert_eq!(reported_methods(source), Vec::<String>::new());
    }

    #[test]
    fn calls_in_coalescing_assignment_and_nested_arguments_are_calls() {
        let source = r#"<?php
final class PathGenerator {
    public function generate(?string $filename = null): string {
        $filename ??= $this->getBasenameOfFilePath();
        return sprintf('%s/%s', $this->getRootPathForEntity(), $filename);
    }

    private function getBasenameOfFilePath(): string {
        return 'file.php';
    }

    private function getRootPathForEntity(): string {
        return '/root';
    }
}
"#;

        assert_eq!(reported_methods(source), Vec::<String>::new());
    }

    #[test]
    fn calls_in_other_expression_positions_are_calls() {
        let source = r#"<?php
final class Calls {
    public function run(): void {
        if ($this->isReady()) {
            echo $this->render();
        }
        $values = ['key' => $this->value(), 'nested' => [$this->nested()]];
        $other = $this->assigned();
        foreach ($values as $value) {
            $result = $value ? $this->fromTernary() : $this->fromElse();
        }
    }

    private function isReady(): bool { return true; }
    private function render(): string { return ''; }
    private function value(): string { return ''; }
    private function nested(): string { return ''; }
    private function assigned(): string { return ''; }
    private function fromTernary(): string { return ''; }
    private function fromElse(): string { return ''; }
    private function neverCalled(): string { return ''; }
}
"#;

        let reported = reported_methods(source);
        assert_eq!(reported.len(), 1, "{reported:?}");
        assert!(reported[0].contains("neverCalled"), "{reported:?}");
    }

    #[test]
    fn nullsafe_static_and_case_insensitive_calls_are_calls() {
        let source = r#"<?php
final class CallForms {
    public function run(): void {
        self::helper();
        $this?->maybeCalled();
        $this->GETBASENAME();
    }

    private static function helper(): void {}
    private function maybeCalled(): void {}
    private function getBasename(): string { return ''; }
    private function trulyUnused(): void {}
}
"#;

        let reported = reported_methods(source);
        assert_eq!(reported.len(), 1, "{reported:?}");
        assert!(reported[0].contains("trulyUnused"), "{reported:?}");
    }

    #[test]
    fn first_class_callables_are_calls() {
        let source = r#"<?php
final class CallableUse {
    public function run(): callable {
        return $this->usedAsCallable(...);
    }

    private function usedAsCallable(): void {}
    private function staticCallable(): void {}

    public function runStatic(): callable {
        return self::staticCallable(...);
    }
}
"#;

        assert_eq!(reported_methods(source), Vec::<String>::new());
    }
}
