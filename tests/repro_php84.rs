use mago_interner::ThreadedInterner;
use mago_lexer::input::Input;
use mago_parser::parse;
use mago_source::{SourceCategory, SourceManager};

#[test]
fn test_php_84_property_hooks_parsing() {
    let code = r#"<?php
class User {
    public string $name {
        get { return $this->name; }
        set { $this->name = $value; }
    }
}
"#;

    let interner = ThreadedInterner::new();
    let source_manager = SourceManager::new(interner.clone());
    let source_id = source_manager.insert_content(
        "test.php".to_string(),
        code.to_string(),
        SourceCategory::UserDefined,
    );

    let source = source_manager.load(&source_id).unwrap();
    let content = interner.lookup(&source.content);
    let input = Input::new(source_id, content.as_bytes());

    let (program, error) = parse(&interner, input);

    if let Some(e) = error {
        panic!("Parsing failed: {:?}", e);
    }

    assert!(
        !program.statements.is_empty(),
        "Program should not be empty"
    );
}
