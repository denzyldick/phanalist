use analyse::Analyse;
use outputs::Format;
pub mod analyse;
pub mod baseline;
pub mod config;
pub mod debug_stats;
pub mod file;
pub mod outputs;
pub mod paths;
pub mod results;
pub mod rules;

pub fn scan(path: String) -> results::Results {
    let output_format = Format::json;
    let config = Analyse::parse_config(path.clone(), &output_format, false);

    let analyze: Analyse = Analyse::new(&config);

    analyze.scan("./src".to_string(), &config, false, &output_format,0,false)
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
