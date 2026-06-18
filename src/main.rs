extern crate exitcode;

use std::path::Path;
use std::process;
use std::str::FromStr;

use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::analyse::Analyse;
use crate::baseline::Baseline;
use crate::engineer::{BlameConfig, EngineerBlame};
use crate::outputs::Format;

mod analyse;
mod baseline;
mod config;
mod debug_stats;
mod engineer;
mod file;
mod outputs;
mod paths;
mod results;
mod rules;

///
/// A static analyser for your PHP project.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(short, long, default_value = "./phanalist.yaml")]
    config: String,
    #[arg(short, long, default_values_t = ["./src".to_string()])]
    src: Vec<String>,
    #[arg(short, long)]
    /// The list of rules to use (by default it is used from config)
    rules: Option<Vec<String>>,
    #[arg(short, long, default_value = "text")]
    /// Possible options: text, json, sarif, codeclimate
    output_format: String,
    #[arg(long)]
    /// Output only summary
    summary_only: bool,
    #[arg(short, long)]
    /// Do not output the results
    quiet: bool,
    #[arg(short, long, action = clap::ArgAction::Count)]
    /// Increase verbosity. Repeat to print each file as it is scanned:
    /// -v main pass, -vv parsing, -vvv indexing
    verbose: u8,
    #[arg(long)]
    /// Print per-rule per-file timing (min/max/avg/p90/p95/p99 + slowest files)
    debug_rule_timing: bool,
    #[arg(long)]
    /// Print per-rule cost/coverage stats (total time, %, violations, files, statements)
    debug_rule_stats: bool,
    #[arg(long)]
    /// Filter results against a baseline file, reporting only new violations
    use_baseline: Option<String>,
    #[arg(long)]
    /// Discard the existing baseline and regenerate it from the current scan (requires --use-baseline)
    update_baseline: bool,
    #[arg(long)]
    /// Attribute violations to engineers via git blame and show a quality report
    blame: bool,
    #[arg(long)]
    /// Show violations introduced since this date (e.g. "30 days", "1 year", "2025-01-01")
    since: Option<String>,
    #[arg(long)]
    /// Show violations until this date (e.g. "2025-06-01")
    until: Option<String>,
    #[arg(long)]
    /// Export engineer chart as PNG/SVG image (requires --blame)
    export_chart: Option<String>,
    #[arg(long)]
    /// Exclude these authors from the engineer report (repeatable)
    exclude_author: Vec<String>,
    #[arg(long, default_value = "0")]
    /// Minimum total violations to include an engineer in the report
    min_violations: u64,
}

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    let args = Args::parse();

    let quiet = args.quiet;

    let paths = args.src;
    for path in paths.iter() {
        if !Path::new(&path).exists() {
            println!("Path {} does not exist", path);
            process::exit(exitcode::IOERR);
        }
    }

    let format = match outputs::Format::from_str(args.output_format.as_str()) {
        Ok(format) => format,
        Err(_) => {
            println!("Invalid input format ({})", args.output_format.as_str());
            process::exit(exitcode::USAGE);
        }
    };

    let mut config = Analyse::parse_config(args.config, &format, quiet);
    if let Some(rules) = args.rules {
        config.enabled_rules = rules;
    }
    let mut analyze = Analyse::new(&config);

    if args.update_baseline && args.use_baseline.is_none() {
        eprintln!("--update-baseline requires --use-baseline <path>");
        process::exit(exitcode::USAGE);
    }

    // In filter mode (use-baseline without update) load the baseline up front.
    let baseline = match (&args.use_baseline, args.update_baseline) {
        (Some(path), false) => match Baseline::load(Path::new(path)) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("Baseline file not found: {path}");
                process::exit(exitcode::USAGE);
            }
            Err(e) => {
                eprintln!("Failed to read baseline {path}: {e}");
                process::exit(exitcode::DATAERR);
            }
        },
        _ => None,
    };

    let mut has_violations = false;
    let mut aggregate = results::Results::default();

    let collect_rule_metrics = args.debug_rule_timing || args.debug_rule_stats;

    if collect_rule_metrics && format != Format::text {
        eprintln!("--debug-rule-timing/--debug-rule-stats only produce output with text format");
    }

    let blame_bar: Option<ProgressBar> = if args.blame && format == Format::text && !quiet {
        let pb = indicatif::ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40}] {pos}/{len} files ({eta})")
                .unwrap()
                .progress_chars("=>"),
        );
        pb.set_message("scanning");
        Some(pb)
    } else {
        None
    };

    // Silence unused warning when --blame is not used
    for path in paths.iter() {
        let mut results = analyze.scan(
            path.clone(),
            &config,
            format != Format::json && !quiet && !args.blame,
            &format,
            args.verbose,
            collect_rule_metrics,
            blame_bar.clone(),
        );

        // Update mode: collect every violation for the new baseline and skip
        // per-path output entirely.
        if args.update_baseline {
            aggregate.files.extend(results.files);
            continue;
        }

        if let Some(ref baseline) = baseline {
            baseline.filter(&mut results);
        }

        if !quiet && !args.blame {
            analyze.output(&mut results, format.clone(), args.summary_only);
        }

        if collect_rule_metrics && format == Format::text {
            if let Some(rt) = &results.rule_timings {
                rt.print_text(
                    &results.codes_count,
                    results.total_files_count,
                    args.debug_rule_timing,
                    args.debug_rule_stats,
                );
            }
        }

        has_violations = has_violations || results.has_any_violations();

        aggregate.files.extend(results.files);
        for (code, count) in results.codes_count {
            *aggregate.codes_count.entry(code).or_insert(0) += count;
        }
        aggregate.total_files_count += results.total_files_count;
    }

    if let Some(ref b) = blame_bar {
        let total = aggregate.total_files_count;
        let blame_total = if args.since.is_some() || args.until.is_some() {
            total * 2
        } else {
            let vio_count = aggregate.files.iter().filter(|(_, v)| !v.is_empty()).count();
            total + vio_count as i64
        };
        b.set_length(blame_total as u64);
        b.set_message("blaming");
    }

    if args.update_baseline {
        let path = args.use_baseline.expect("validated above");
        let baseline = Baseline::from_results(&aggregate);
        let path_clone = path.clone();
        match baseline.save(&std::path::PathBuf::from(path_clone)) {
            Ok(()) => {
                if !quiet && format == Format::text {
                    println!(
                        "Baseline written to {} ({} entries).",
                        path,
                        baseline.violations.len()
                    );
                }
                process::exit(exitcode::OK);
            }
            Err(e) => {
                eprintln!("Failed to write baseline {path}: {e}");
                process::exit(exitcode::CANTCREAT);
            }
        }
    }

    // Git repo discovered after scanning to avoid FFI/library conflicts
    let blame_git_repo = if args.blame {
        let src_path = Path::new(&paths[0]);
        match git2::Repository::discover(src_path) {
            Ok(r) => Some(r),
            Err(_) => {
                let has_git = src_path.ancestors().any(|d| d.join(".git").exists());
                if has_git {
                    eprintln!("The .git directory found in parent directories of {} appears corrupt or invalid.", src_path.display());
                } else {
                    eprintln!("No .git directory found in {}. --blame requires a git repository.", src_path.display());
                }
                process::exit(exitcode::USAGE);
            }
        }
    } else {
        None
    };

    if let Some(ref repo) = blame_git_repo {
        let repo_root = match repo.workdir() {
            Some(p) => p.to_path_buf(),
            None => {
                eprintln!("Repository has no working directory (bare repo).");
                process::exit(exitcode::USAGE);
            }
        };

        if args.since.is_some() || args.until.is_some() {
            if format == Format::text {
                eprintln!(
                    "{}",
                    "Notice: Running historical analysis with --since/--until. \
                     This will re-analyse changed files from the git history. \
                     May take a while depending on repo size."
                        .yellow()
                );
            }
        }

        let blame_config = BlameConfig {
            since: args.since.clone(),
            until: args.until.clone(),
            exclude_authors: args.exclude_author.clone(),
            min_violations: args.min_violations,
        };

        let engineer = match EngineerBlame::new(&repo_root, &blame_config) {
            Ok(e) => e,
            Err(msg) => {
                eprintln!("{msg}");
                process::exit(exitcode::DATAERR);
            }
        };

        let report = engineer.attribute_violations(
            &aggregate,
            &analyze,
            &config,
            &format,
            args.verbose,
            &blame_bar,
        );

        aggregate.engineer_report = Some(report.clone());

        if let Some(ref b) = blame_bar {
            b.finish_with_message("blame done");
        }

        if !quiet {
            if format == Format::text {
                outputs::chart::print_engineer_report(&report, &args.since);
            } else if format == Format::json {
                println!("{}", serde_json::to_string_pretty(&aggregate).unwrap());
            }
        }

        if let Some(chart_path) = &args.export_chart {
            match outputs::chart::export_chart_image(&report, chart_path, &args.since) {
                Ok(()) => {
                    if format == Format::text {
                        println!("Chart exported to {}", chart_path.green().bold());
                    }
                }
                Err(e) => {
                    eprintln!("Failed to export chart: {e}");
                }
            }
        }
    }

    if has_violations {
        process::exit(exitcode::SOFTWARE);
    } else {
        process::exit(exitcode::OK);
    }
}
