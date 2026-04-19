use std::borrow::Cow;

use bumpalo::Bump;
use mago_database::file::File;
use mago_syntax::parser::parse_file;

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

    let arena = Bump::new();
    let file = File::ephemeral(Cow::Borrowed("test.php"), Cow::Owned(code.to_string()));

    let program = parse_file(&arena, &file);

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
