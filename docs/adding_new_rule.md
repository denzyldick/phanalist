# Tutorial: Adding a New Rule

Adding a new rule to Phanalist is straightforward. Whether you are a junior engineer contributing your first check or an experienced developer adding a custom analyzer, follow these step-by-step instructions.

---

### Step 1: Choose a Rule Code
Pick the next available rule code (for example, `E1234`). You can use `E0000` (`src/rules/e0.rs`) as your starting blueprint.

### Step 2: Create the Rule Module in Rust
Duplicate the example rule file:
```bash
cp src/rules/e0.rs src/rules/e1234.rs
```

Open `src/rules/e1234.rs` and update the `CODE` and `DESCRIPTION` constants:
```rust
pub(crate) static CODE: &str = "E1234";
static DESCRIPTION: &str = "Your clear, descriptive rule title";
```

### Step 3: Implement Validation Logic
In `src/rules/e1234.rs`, implement the `validate` function. Your rule will receive AST statements or expressions, analyze them, and return a vector of `Violation` objects.

Use `Message::new(id, template)` where `id` is a stable slug (used for baseline tracking) and template uses `{placeholders}` for dynamic values:
```rust
use crate::results::{Message, Violation};

let message = Message::new(
    "E1234:short-slug",
    "Explain the problem clearly with {name}.",
)
.arg("name", name.to_string());

violations.push(self.new_violation(file, message, span));
```

### Step 4: Write Unit Tests
Every rule must include tests in the `mod tests` block at the bottom of `src/rules/e1234.rs`, asserting against expected findings and rendered messages:
```rust
#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;
    use super::*;

    #[test]
    fn test_rule_detection() {
        let violations = analyze_file_for_rule("e1234/violation_example.php", CODE);
        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().message.render(),
            "Expected violation message here.".to_string()
        );
    }
}
```

### Step 5: Create PHP Fixture Files
Create fixture PHP files under `src/rules/examples/e1234/` containing bad and good examples used by your tests.

### Step 6: Enable the Rule in the Engine
Register your new rule module in `src/rules/mod.rs`:
```rust
pub mod e1234;

pub fn all_rules() -> HashMap<String, Box<dyn Rule>> {
    // ...
    add_rule(&mut rules, Box::default() as Box<e1234::Rule>);
    // ...
}
```

### Step 7: Write Rule Documentation (`e1234.md`)
Create `src/rules/examples/e1234/e1234.md`. Every rule documentation file must follow the project standard, making it easy for junior engineers to understand:
1. **Title & Plain-English Summary:** What does the rule check?
2. **Why it matters:** Real-world impact and consequences.
3. **Bad Example:** PHP code snippet showing a violation.
4. **Good Example:** PHP code snippet showing correct implementation.
5. **What the rule checks:** Specific conditions inspected.
6. **When it does not report:** Edge cases and exceptions.
7. **How to fix:** Actionable instructions.
8. **Configuration:** Default status and config schema.
9. **Junior Engineer's Rule of Thumb:** A concise, memorable takeaway.

### Step 8: Update `README.md`
Add your new rule to the Rules table in `README.md` with a link to its markdown documentation.

### Step 9: Verify and Submit PR
Run cargo test to ensure all tests pass:
```bash
cargo test
```
Submit your pull request!
