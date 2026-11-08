# Would you like to contribute?

Go ahead!

### Bugs.
When you find a bug in the system, I would appreciate it if you could open a new issue. I will do my best to fix the problem in my free time. Or if
you can solve it yourself. You can do that. All PR should target the `main` branch. There is currently no `staging` or `development` branch. I
want to keep everything simple.


## Adding new rule

[E0000 Example Rule](src/rules/e0.rs) can be used as a base for new rules. Please follow the next steps to add a new rule:

1. Pickup available code for a new rule; let's say it is `E1234`.
2. Create a new rule module:
    ```bash
    cp src/rules/e0.rs src/rules/e1234.rs
    ```
3. Update the `CODE` and `DESCRIPTION` constants in the new `src/rules/e1234.rs`:
    ```rust
    ...
    pub(crate) static CODE: &str = "E1234";
    static DESCRIPTION: &str = "Your rule description";
    ...
    ```
4. Create PHP examples for the new rule:
    ```bash
    cp -r src/rules/examples/e0 src/rules/examples/e1234
    ```
5. Implement the `validate` function in `src/rules/e1234.rs`, and cover it with tests in `mod tests`.
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
7. Add the rule to the matching section of the Rules Reference in `README.md` and link its documentation.
8. Create `src/rules/examples/e1234/e1234.md`.
9. Document the rule using the standard sections in `docs/adding_new_rule.md`.
10. Done! Submit a new PR with the new rule.
11. Wait for it to be released.
12. Repeat for your next rule.

### Documentation
_The idea I have in my mind is that all the rules should be documented for an inexperienced PHP developer. If you have experience with PHP
and an eye for beauty, your help is welcome._ 
