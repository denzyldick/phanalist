use std::collections::HashMap;
use std::time::Duration;

use cli_table::{format::Justify, Cell, Style, Table};

/// One rule's metrics on a single file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileRuleMetric {
    /// Summed flatten + validate time on this file (including the do_validate check).
    pub duration: Duration,
    /// Whether do_validate() engaged on this file.
    pub validated: bool,
    /// Number of statement nodes the rule actually inspected.
    pub statements: usize,
}

/// Per-file accumulation, owned and returned by `analyse_file`.
/// Keyed by rule code.
pub type FileTimings = HashMap<String, FileRuleMetric>;

/// Collected across a scan: one (file_path, metric) sample per file per rule.
#[derive(Debug, Default, Clone)]
pub struct RuleTimings {
    pub per_file: HashMap<String, Vec<(String, FileRuleMetric)>>,
}

/// Computed, display-ready view for one rule.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleStat {
    pub code: String,
    pub samples: usize,
    // per-file timing
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    // cost / coverage
    pub total: Duration,
    pub pct_of_total: f64,
    pub files_validated: usize,
    pub files_skipped: usize,
    pub statements: usize,
    pub violations: i64,
    pub slowest: Vec<(String, Duration)>,
}

impl RuleTimings {
    /// Append one sample per rule from a finished file.
    pub fn merge_file(&mut self, file_path: String, timings: FileTimings) {
        for (code, metric) in timings {
            self.per_file
                .entry(code)
                .or_default()
                .push((file_path.clone(), metric));
        }
    }

    /// Render the debug section to stdout. `show_timing` prints the per-file
    /// timing table + slowest-files listing; `show_stats` prints the cost table.
    pub fn print_text(
        &self,
        codes_count: &HashMap<String, i64>,
        total_files: i64,
        show_timing: bool,
        show_stats: bool,
    ) {
        let stats = self.compute(codes_count);
        if stats.is_empty() {
            return;
        }

        println!();
        println!("Rule debug ({total_files} files):");

        if show_timing {
            let mut by_p99 = stats.clone();
            by_p99.sort_by_key(|s| std::cmp::Reverse(s.p99));

            let rows: Vec<Vec<_>> = by_p99
                .iter()
                .map(|s| {
                    vec![
                        s.code.as_str().cell(),
                        s.samples.cell().justify(Justify::Right),
                        format!("{:.2?}", s.min).cell().justify(Justify::Right),
                        format!("{:.2?}", s.avg).cell().justify(Justify::Right),
                        format!("{:.2?}", s.p90).cell().justify(Justify::Right),
                        format!("{:.2?}", s.p95).cell().justify(Justify::Right),
                        format!("{:.2?}", s.p99).cell().justify(Justify::Right),
                        format!("{:.2?}", s.max).cell().justify(Justify::Right),
                    ]
                })
                .collect();

            let table = rows
                .table()
                .title(vec![
                    "Rule".cell().bold(true),
                    "Files".cell().bold(true),
                    "Min".cell().bold(true),
                    "Avg".cell().bold(true),
                    "p90".cell().bold(true),
                    "p95".cell().bold(true),
                    "p99".cell().bold(true),
                    "Max".cell().bold(true),
                ])
                .bold(true);
            println!("{}", table.display().unwrap());

            let mut by_total = stats.clone();
            by_total.sort_by_key(|s| std::cmp::Reverse(s.total));
            println!("Slowest files per rule:");
            for s in &by_total {
                println!("  {}:", s.code);
                for (path, dur) in &s.slowest {
                    println!("    {:>10}  {}", format!("{dur:.2?}"), path);
                }
            }
        }

        if show_stats {
            let mut by_total = stats.clone();
            by_total.sort_by_key(|s| std::cmp::Reverse(s.total));

            let rows: Vec<Vec<_>> = by_total
                .iter()
                .map(|s| {
                    vec![
                        s.code.as_str().cell(),
                        format!("{:.2?}", s.total).cell().justify(Justify::Right),
                        format!("{:.1}%", s.pct_of_total).cell().justify(Justify::Right),
                        s.violations.cell().justify(Justify::Right),
                        format!("{}/{}", s.files_validated, s.files_skipped)
                            .cell()
                            .justify(Justify::Right),
                        s.statements.cell().justify(Justify::Right),
                    ]
                })
                .collect();

            let table = rows
                .table()
                .title(vec![
                    "Rule".cell().bold(true),
                    "Total".cell().bold(true),
                    "% Total".cell().bold(true),
                    "Violations".cell().bold(true),
                    "Valid/Skip".cell().bold(true),
                    "Statements".cell().bold(true),
                ])
                .bold(true);
            println!("{}", table.display().unwrap());
        }
    }

    /// Aggregate into per-rule stats. `codes_count` supplies violation counts.
    pub fn compute(&self, codes_count: &HashMap<String, i64>) -> Vec<RuleStat> {
        let grand_total_nanos: u128 = self
            .per_file
            .values()
            .flat_map(|v| v.iter())
            .map(|(_, m)| m.duration.as_nanos())
            .sum::<u128>()
            .max(1);

        let mut stats = Vec::new();
        for (code, samples) in &self.per_file {
            let n = samples.len();
            if n == 0 {
                continue;
            }

            let mut durations: Vec<Duration> = samples.iter().map(|(_, m)| m.duration).collect();
            durations.sort();

            let total: Duration = durations.iter().sum();
            let avg = total / n as u32;
            let min = durations[0];
            let max = durations[n - 1];

            // Nearest-rank percentile: rank = ceil(p/100 * n), clamped to [1, n].
            let percentile = |pct: f64| -> Duration {
                let rank = ((pct / 100.0) * n as f64).ceil() as usize;
                let idx = rank.clamp(1, n) - 1;
                durations[idx]
            };

            let files_validated = samples.iter().filter(|(_, m)| m.validated).count();
            let statements = samples.iter().map(|(_, m)| m.statements).sum();

            let mut slowest: Vec<(String, Duration)> = samples
                .iter()
                .map(|(path, m)| (path.clone(), m.duration))
                .collect();
            slowest.sort_by_key(|&(_, d)| std::cmp::Reverse(d));
            slowest.truncate(5);

            let pct_of_total = total.as_nanos() as f64 / grand_total_nanos as f64 * 100.0;

            stats.push(RuleStat {
                code: code.clone(),
                samples: n,
                min,
                max,
                avg,
                p90: percentile(90.0),
                p95: percentile(95.0),
                p99: percentile(99.0),
                total,
                pct_of_total,
                files_validated,
                files_skipped: n - files_validated,
                statements,
                violations: codes_count.get(code).copied().unwrap_or(0),
                slowest,
            });
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(ms: u64, validated: bool, statements: usize) -> FileRuleMetric {
        FileRuleMetric {
            duration: Duration::from_millis(ms),
            validated,
            statements,
        }
    }

    fn timings_with(samples: Vec<(&str, FileRuleMetric)>) -> RuleTimings {
        let mut t = RuleTimings::default();
        let per: Vec<(String, FileRuleMetric)> =
            samples.into_iter().map(|(p, m)| (p.to_string(), m)).collect();
        t.per_file.insert("E0001".to_string(), per);
        t
    }

    fn only_stat(stats: Vec<RuleStat>) -> RuleStat {
        assert_eq!(stats.len(), 1);
        stats.into_iter().next().unwrap()
    }

    #[test]
    fn percentiles_on_1_to_100ms() {
        let mut t = RuleTimings::default();
        let per: Vec<(String, FileRuleMetric)> = (1..=100u64)
            .map(|ms| (format!("f{ms}.php"), metric(ms, true, 0)))
            .collect();
        t.per_file.insert("E0001".to_string(), per);

        let stat = only_stat(t.compute(&HashMap::new()));
        assert_eq!(stat.samples, 100);
        assert_eq!(stat.min, Duration::from_millis(1));
        assert_eq!(stat.max, Duration::from_millis(100));
        assert_eq!(stat.avg, Duration::from_micros(50_500)); // 5050ms / 100
        assert_eq!(stat.p90, Duration::from_millis(90));
        assert_eq!(stat.p95, Duration::from_millis(95));
        assert_eq!(stat.p99, Duration::from_millis(99));
        assert_eq!(stat.total, Duration::from_millis(5050));
    }

    #[test]
    fn percentiles_single_sample() {
        let t = timings_with(vec![("a.php", metric(7, true, 3))]);
        let stat = only_stat(t.compute(&HashMap::new()));
        assert_eq!(stat.samples, 1);
        assert_eq!(stat.min, Duration::from_millis(7));
        assert_eq!(stat.max, Duration::from_millis(7));
        assert_eq!(stat.avg, Duration::from_millis(7));
        assert_eq!(stat.p90, Duration::from_millis(7));
        assert_eq!(stat.p95, Duration::from_millis(7));
        assert_eq!(stat.p99, Duration::from_millis(7));
    }

    #[test]
    fn percentiles_three_samples() {
        let t = timings_with(vec![
            ("a.php", metric(10, true, 1)),
            ("b.php", metric(20, true, 1)),
            ("c.php", metric(30, true, 1)),
        ]);
        let stat = only_stat(t.compute(&HashMap::new()));
        assert_eq!(stat.min, Duration::from_millis(10));
        assert_eq!(stat.max, Duration::from_millis(30));
        assert_eq!(stat.avg, Duration::from_millis(20));
        assert_eq!(stat.p90, Duration::from_millis(30));
        assert_eq!(stat.p95, Duration::from_millis(30));
        assert_eq!(stat.p99, Duration::from_millis(30));
    }

    #[test]
    fn coverage_and_violations() {
        let t = timings_with(vec![
            ("a.php", metric(10, true, 4)),
            ("b.php", metric(0, false, 0)),
            ("c.php", metric(5, true, 2)),
        ]);
        let mut codes = HashMap::new();
        codes.insert("E0001".to_string(), 9);

        let stat = only_stat(t.compute(&codes));
        assert_eq!(stat.files_validated, 2);
        assert_eq!(stat.files_skipped, 1);
        assert_eq!(stat.statements, 6);
        assert_eq!(stat.violations, 9);
        assert_eq!(stat.total, Duration::from_millis(15));
    }

    #[test]
    fn pct_of_total_sums_to_100() {
        let mut t = RuleTimings::default();
        t.per_file.insert(
            "E0001".to_string(),
            vec![("a.php".to_string(), metric(30, true, 0))],
        );
        t.per_file.insert(
            "E0002".to_string(),
            vec![("a.php".to_string(), metric(10, true, 0))],
        );
        let stats = t.compute(&HashMap::new());
        let sum: f64 = stats.iter().map(|s| s.pct_of_total).sum();
        assert!((sum - 100.0).abs() < 0.001, "pct sum was {sum}");
        let e1 = stats.iter().find(|s| s.code == "E0001").unwrap();
        assert!((e1.pct_of_total - 75.0).abs() < 0.001);
    }

    #[test]
    fn slowest_truncates_to_5_descending() {
        let t = timings_with(vec![
            ("a.php", metric(1, true, 0)),
            ("b.php", metric(6, true, 0)),
            ("c.php", metric(3, true, 0)),
            ("d.php", metric(5, true, 0)),
            ("e.php", metric(2, true, 0)),
            ("f.php", metric(4, true, 0)),
        ]);
        let stat = only_stat(t.compute(&HashMap::new()));
        assert_eq!(stat.slowest.len(), 5);
        let durations: Vec<u64> = stat.slowest.iter().map(|(_, d)| d.as_millis() as u64).collect();
        assert_eq!(durations, vec![6, 5, 4, 3, 2]);
        assert_eq!(stat.slowest[0].0, "b.php");
    }
}
