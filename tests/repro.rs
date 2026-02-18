use phanalist;

#[test]
fn test_php84_hooks() {
    // We expect this to likely fail or panic with the current parser
    let result = std::panic::catch_unwind(|| {
        let _ = phanalist::scan("phanalist.yaml".to_string());
    });

    // For now, just printing result to see what happens
    println!("Scan result: {:?}", result);
}
