use analyse::Analyse;
use output::Format;
mod analyse;
mod config;
mod file;
mod output;
mod results;
mod rules;

pub fn scan(path: String) -> results::Results {
    let output_format = Format::json;
    let config = Analyse::parse_config(path.clone(), &output_format, false);

    let analyze: Analyse = Analyse::new(&config);

    analyze.scan("./".to_string(), &config, false)
}

#[test]
fn run() {
    let violations = scan(String::from("./rules/examples/phanalist.yaml"));

    // @todo write a more usefull test.
    assert_eq!(109, violations.total_files_count)
}
