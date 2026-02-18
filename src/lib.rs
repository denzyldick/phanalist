use analyse::Analyse;
use outputs::Format;
pub mod analyse;
pub mod config;
pub mod file;
pub mod outputs;
pub mod results;
pub mod rules;

pub fn scan(path: String) -> results::Results {
    let output_format = Format::json;
    let config = Analyse::parse_config(path.clone(), &output_format, false);

    let analyze: Analyse = Analyse::new(&config);

    analyze.scan("./".to_string(), &config, false, &output_format)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run() {
        let violations = scan(String::from("./src/rules/examples/phanalist.yaml"));

        assert_ne!(0, violations.total_files_count)
    }
}
