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

#[cfg(feature = "chart-image")]
pub fn export_chart_image(
    report: &EngineerReport,
    path: &str,
    since: &Option<String>,
) -> Result<(), String> {
    use plotters::prelude::*;

    let mut entries: Vec<(&String, &EngineerEntry)> = report.iter().collect();
    entries.sort_by_key(|(_, e)| std::cmp::Reverse(e.total_introduced + e.total_fixed));

    if entries.is_empty() {
        return Err("No data to chart".to_string());
    }

    let max_total = entries
        .iter()
        .map(|(_, e)| e.total_introduced + e.total_fixed)
        .max()
        .unwrap_or(1)
        .max(1) as i32;

    let title = match since {
        Some(s) => format!("Engineer Quality Report (since {s})"),
        None => "Engineer Quality Report".to_string(),
    };

    let (width, height) = (800, 50 + entries.len() * 60);
    let ext = path.split('.').last().unwrap_or("png");

    if ext == "svg" {
        let root = SVGBackend::new(path, (width as u32, height as u32)).into_drawing_area();
        draw_chart_on_root(root, &entries, &title, max_total)?;
    } else {
        let root = BitMapBackend::new(path, (width as u32, height as u32)).into_drawing_area();
        draw_chart_on_root(root, &entries, &title, max_total)?;
    }

    Ok(())
}

#[cfg(feature = "chart-image")]
fn draw_chart_on_root<DB: plotters::prelude::DrawingBackend>(
    root: plotters::prelude::DrawingArea<DB, plotters::coord::Shift>,
    entries: &[(&String, &EngineerEntry)],
    title: &str,
    max_total: i32,
) -> Result<(), String>
where
    DB::ErrorType: 'static,
{
    use plotters::prelude::*;

    root.fill(&WHITE).map_err(|e| format!("Cannot create image: {e}"))?;

    let mut chart = ChartBuilder::on(&root)
        .caption(title, ("sans-serif", 24).into_font())
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            0..(max_total + 2),
            (0..entries.len()).into_segmented(),
        )
        .map_err(|e| format!("Chart build error: {e}"))?;

    chart
        .configure_mesh()
        .x_labels(5)
        .y_labels(entries.len())
        .y_label_formatter(&|y| {
            if let SegmentValue::Exact(i) = y {
                if *i < entries.len() {
                    entries[*i].0.clone()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        })
        .draw()
        .map_err(|e| format!("Mesh error: {e}"))?;

    for (i, (author, entry)) in entries.iter().enumerate() {
        let total = (entry.total_introduced + entry.total_fixed) as i32;
        let fixed = entry.total_fixed as i32;

        if fixed > 0 {
            let y0 = SegmentValue::Exact(i);
            let color = GREEN;
            chart
                .draw_series(std::iter::once(Rectangle::new(
                    [(0, y0.clone()), (fixed, y0)],
                    color.filled(),
                )))
                .map_err(|e| format!("Draw error: {e}"))?
                .label(format!("{} fixed", author))
                .legend(move |(x, y)| {
                    Rectangle::new([(x, y - 5), (x + 10, y + 5)], color.filled())
                });
        }

        if entry.total_introduced > 0 {
            let y0 = SegmentValue::Exact(i);
            let color = RED;
            chart
                .draw_series(std::iter::once(Rectangle::new(
                    [(fixed, y0.clone()), (total, y0)],
                    color.filled(),
                )))
                .map_err(|e| format!("Draw error: {e}"))?
                .label(format!("{} introduced", author))
                .legend(move |(x, y)| {
                    Rectangle::new([(x, y - 5), (x + 10, y + 5)], color.filled())
                });
        }
    }

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()
        .map_err(|e| format!("Legend error: {e}"))?;

    root.present().map_err(|e| format!("Present error: {e}"))?;
    Ok(())
}

#[cfg(not(feature = "chart-image"))]
pub fn export_chart_image(
    _report: &EngineerReport,
    _path: &str,
    _since: &Option<String>,
) -> Result<(), String> {
    Err("Image export requires the 'chart-image' feature: cargo build --features chart-image".to_string())
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

    #[test]
    fn test_export_chart_image_no_data() {
        let report: EngineerReport = std::collections::HashMap::new();
        let result = export_chart_image(&report, "test.png", &None);
        assert!(result.is_err());
    }
}
