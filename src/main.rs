extern crate exitcode;

use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

use clap::Parser;

use crate::analyse::Analyse;
use crate::baseline::Baseline;
use crate::outputs::Format;

mod analyse;
mod baseline;
mod config;
mod debug_stats;
mod file;
mod outputs;
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

    for path in paths.iter() {
        let mut results = analyze.scan(
            path.clone(),
            &config,
            format != Format::json && !quiet,
            &format,
            args.verbose,
            collect_rule_metrics,
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

        if !quiet {
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
    }

    if args.update_baseline {
        let path = args.use_baseline.expect("validated above");
        let baseline = Baseline::from_results(&aggregate);
        match baseline.save(&PathBuf::from(&path)) {
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

    if has_violations {
        process::exit(exitcode::SOFTWARE);
    } else {
        process::exit(exitcode::OK);
    }
}
