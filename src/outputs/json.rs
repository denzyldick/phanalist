use crate::results::Results;

use super::OutputFormatter;

pub struct Json {}
impl OutputFormatter for Json {
    fn output(results: &mut Results) {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    }
}