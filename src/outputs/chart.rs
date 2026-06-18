use cli_table::{format::Justify, Cell, Style, Table};
use colored::Colorize;

use crate::results::{EngineerEntry, EngineerReport, RuleChange};

pub fn print_engineer_report(report: &EngineerReport, since: &Option<String>) {
    if report.is_empty() {
        println!("{}", "No engineer data to report.".yellow());
        return;
    }

    let mut entries: Vec<(&String, &EngineerEntry)> = report.iter().collect();
    entries.sort_by_key(|(_, e)| std::cmp::Reverse(e.total_introduced + e.total_fixed));

    let title = match since {
        Some(s) => format!("Engineer Quality Report (since {s})"),
        None => "Engineer Quality Report".to_string(),
    };

    println!("\n{}", title.bold().underline());
    println!();

    let mut table_rows = vec![];

    for (author, entry) in &entries {
        let net_str = if entry.net > 0 {
            format!("+{}", entry.net).green().to_string()
        } else if entry.net < 0 {
            format!("{}", entry.net).red().to_string()
        } else {
            "0".to_string()
        };

        table_rows.push(vec![
            author.as_str().cell(),
            entry.total_fixed.cell().justify(Justify::Right),
            entry.total_introduced.cell().justify(Justify::Right),
            net_str.cell().justify(Justify::Right),
        ]);
    }

    let table = table_rows.table().title(vec![
        "Engineer".cell().bold(true),
        "Fixed (✓)".cell().bold(true),
        "Introduced (✗)".cell().bold(true),
        "Net".cell().bold(true),
    ]).bold(true);

    println!("{}", table.display().unwrap());
    println!();

    println!("{}", "Per-rule breakdown:".bold().underline());
    println!();

    for (author, entry) in &entries {
        if entry.rules.is_empty() {
            continue;
        }

        let mut rules: Vec<(&String, &RuleChange)> = entry.rules.iter().collect();
        rules.sort_by_key(|(_, rc)| std::cmp::Reverse(rc.introduced + rc.fixed));

        println!("  {}:", author.yellow().bold());
        for (rule, rc) in rules {
            let mut parts = vec![];
            if rc.fixed > 0 {
                parts.push(format!("{} fixed", rc.fixed.to_string().green()));
            }
            if rc.introduced > 0 {
                parts.push(format!("{} introduced", rc.introduced.to_string().red()));
            }
            let desc = parts.join(", ");
            println!("    {}  {}", rule.dimmed(), desc);
        }
        println!();
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::RuleChange;

    fn make_report() -> EngineerReport {
        let mut report: EngineerReport = std::collections::HashMap::new();
        let mut alice = EngineerEntry::default();
        alice.total_fixed = 10;
        alice.total_introduced = 3;
        alice.net = 7;
        alice.rules.insert("E001".to_string(), RuleChange { fixed: 8, introduced: 1 });
        alice.rules.insert("E002".to_string(), RuleChange { fixed: 2, introduced: 2 });
        report.insert("Alice".to_string(), alice);

        let mut bob = EngineerEntry::default();
        bob.total_fixed = 1;
        bob.total_introduced = 20;
        bob.net = -19;
        bob.rules.insert("E001".to_string(), RuleChange { fixed: 1, introduced: 15 });
        report.insert("Bob".to_string(), bob);

        report
    }

    #[test]
    fn test_print_engineer_report_empty() {
        let report: EngineerReport = std::collections::HashMap::new();
        // Should not panic
        print_engineer_report(&report, &None);
    }

    #[test]
    fn test_print_engineer_report_with_data() {
        let report = make_report();
        // Should not panic
        print_engineer_report(&report, &Some("30 days".to_string()));
    }

    #[test]
    fn test_print_engineer_report_title_with_since() {
        // Just verify formatting doesn't crash
        let report = make_report();
        print_engineer_report(&report, &Some("2025-01-01".to_string()));
    }

}
