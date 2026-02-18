use crate::file::File;
use crate::results::Violation;
use mago_ast::Statement;
use mago_span::HasSpan;

static CODE: &str = "E0005";
static DESCRIPTION: &str = "Capitalized class name";

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

        if let Statement::Class(class) = statement {
            let name = file.interner.lookup(&class.name.value);
            if !name.chars().next().unwrap().is_uppercase() {
                let suggestion = format!("The class name {} is not capitalized. The first letter of the name of the class should be in uppercase.", name);
                violations.push(self.new_violation(file, suggestion, class.name.span()))
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
            violations.first().unwrap().suggestion,
            "The class name nonCapitalized is not capitalized. The first letter of the name of the class should be in uppercase.".to_string()
        );
    }

    #[test]
    fn capitalized_classname() {
        let violations = analyze_file_for_rule("e5/capitalized_classname.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
