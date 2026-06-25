use mago_span::HasSpan;
use mago_syntax::cst::Statement;

use crate::file::File;
use crate::results::{Message, Violation};

pub(crate) static CODE: &str = "E0005";
static DESCRIPTION: &str = "Capitalized class name";

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
            let name = class.name.value;
            let name_str = std::str::from_utf8(name).unwrap_or_default();
            if let Some(first) = name_str.chars().next() {
                if !first.is_uppercase() {
                    let message = Message::new(
                        "E0005:class-name-not-capitalized",
                        "The class name {name} is not capitalized. The first letter of the name of the class should be in uppercase.",
                    )
                    .arg("name", name_str.to_string());
                    violations.push(self.new_violation(file, message, class.name.span()))
                }
            }
        };

        violations
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn non_capitalized_classname() {
        let violations = analyze_file_for_rule("e5/non_capitalized_classname.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().message.render(),
            "The class name nonCapitalized is not capitalized. The first letter of the name of the class should be in uppercase.".to_string()
        );
    }

    #[test]
    fn capitalized_classname() {
        let violations = analyze_file_for_rule("e5/capitalized_classname.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
