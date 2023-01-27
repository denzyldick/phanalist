use php_parser_rs::lexer::token::Span;

pub fn opening_tag(t: Span) {
    if t.line > 1 {
        println!("The opening tag <?php is not on the right line. This should always be the first line in a PHP file.");
    }

    if t.column > 1 {
         println!(
            "The opening tag doesn't start at the right column: {}.",
            t.column
        );
    }
}
