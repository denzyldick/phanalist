use mago_allocator::prelude::LocalArena;
use mago_database::file::File;
use mago_syntax::parser::parse_file_content;

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

    let arena = LocalArena::new();
    let file = File::ephemeral("test.php".as_bytes().into(), code.to_string().into_bytes().into());

    let program = parse_file_content(&arena, file.id, file.contents.as_ref());

    assert!(
        !program.has_errors(),
        "Parsing failed: {:?}",
        program.errors
    );
    assert!(
        !program.statements.is_empty(),
        "Program should not be empty"
    );
}
