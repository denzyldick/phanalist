use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let examples_dir = manifest_dir.join("src/rules/examples");

    println!("cargo:rerun-if-changed=src/rules/examples");

    let mut docs = Vec::new();
    if let Ok(entries) = fs::read_dir(&examples_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let md = entry.path().join(format!("{name}.md"));
            if md.is_file() {
                let key = name.strip_prefix('e').unwrap_or(&name).to_string();
                docs.push((key, md));
            }
        }
    }
    docs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut generated = String::from(
        "pub fn embedded_rule_doc(rule_number: &str) -> Option<&'static str> {\n    match rule_number {\n",
    );
    for (name, path) in docs {
        generated.push_str(&format!(
            "        {:?} => Some(include_str!({:?})),\n",
            name,
            path.to_string_lossy()
        ));
    }
    generated.push_str("        _ => None,\n    }\n}\n");

    let out_dir = Path::new(&env::var("OUT_DIR").unwrap()).join("embedded_docs.rs");
    fs::write(out_dir, generated).unwrap();
}
