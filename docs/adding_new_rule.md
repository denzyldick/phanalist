## Adding new rule

[E0000 Example Rule](../src/rules/e0.rs) can be used as a base for any new rules. Please follow the next steps to add a new rule:

1. Pickup available code for a new rule, lets say it is `E1234`.
2. Create a new rule module:
    ```bash
    cp src/rules/e0.rs src/rules/e1234.rs
    ```
3. Update `CODE` and `DESCRIPTION` variables in the new `src/rules/e1234.rs`:
    ```rust
    ...
    static CODE: &str = "E1234";
    static DESCRIPTION: &str = "Your rule description";
    ...
    ```
4. Create PHP examples for the new rule:
    ```bash
    cp -r src/rules/examples/e0 src/rules/examples/e1234
    ```
5. Implement the `validate` function in `src/rules/e1234.rs`, returning a `Violation` for each problem found. Build the message with `Message::new(id, template)`. The `id` is a stable slug (used as a key, e.g. by the baseline), so keep it fixed even if you reword the text. Put dynamic values in `{placeholder}` args rather than formatting them into the id or template text:
    ```rust
    use crate::results::{Message, Violation};

    let message = Message::new(
        "E1234:short-slug",
        "Explain the problem with {name}.",
    )
    .arg("name", name.to_string());
    violations.push(self.new_violation(file, message, span));
    ```
    Cover it with tests in `mod tests`, asserting on `violation.message.render()`.
6. Enable the new rule in `src/rules/mod.rs`:
    ```rust
    ...
    pub mod e1234;
    ...
    pub fn all_rules() -> HashMap<String, Box<dyn Rule>> {
        ...
        add_rule(&mut rules, Box::default() as Box<e1234::Rule>);
        ...
    }
    ```
7. Update `README.md` with new rule details.
8. Done! Submit a new PR with new rule.