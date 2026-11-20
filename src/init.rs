//! Guided first-run setup for phanalist.
//!
//! `init` never runs a normal analysis except for the explicit baseline step.
//! It writes `phanalist.yaml`, optionally creates a baseline, patches an
//! existing PHPUnit configuration, and writes a GitHub Actions workflow.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::Args;
use serde_json::Value as JsonValue;

use crate::analyse::Analyse;
use crate::baseline::Baseline;
use crate::config::{Config, TestGenerationConfig};
use crate::generated_tests;
use crate::outputs::Format;
use crate::resource::ResourceClass;
use crate::results::Results;
use crate::rules;
use crate::verification::FindingClass;

const DEFAULT_BASELINE: &str = ".phanalist-baseline.json";
const DEFAULT_TEST_DIR: &str = "tests/phanalist-generated";

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Accept shown defaults in non-interactive environments
    #[arg(long)]
    pub yes: bool,
    /// Overwrite an existing phanalist.yaml (a .bak copy is written first)
    #[arg(long)]
    pub force: bool,

    /// Directories to scan (repeatable)
    #[arg(long = "src", value_name = "DIR")]
    pub src: Vec<String>,
    /// Paths or globs to exclude (repeatable)
    #[arg(long = "exclude", value_name = "PATH")]
    pub exclude: Vec<String>,
    /// Rule preset: all, minimal, verification, resource, custom
    #[arg(long, value_name = "PRESET")]
    pub preset: Option<String>,
    /// Explicitly enable a rule code (repeatable)
    #[arg(long = "enable-rule", value_name = "CODE")]
    pub enable_rules: Vec<String>,
    /// Explicitly disable a rule code (repeatable)
    #[arg(long = "disable-rule", value_name = "CODE")]
    pub disable_rules: Vec<String>,
    /// Set a rule value, e.g. E0031.timeout_ms=2000 (repeatable)
    #[arg(long = "set", value_name = "RULE.KEY=VALUE")]
    pub set: Vec<String>,
    /// Report all tainted findings, including parameter-derived leads
    #[arg(long = "taint-all")]
    pub taint_all: bool,
    /// Report only request-rooted tainted findings
    #[arg(long = "request-only")]
    pub request_only: bool,

    /// Default output format
    #[arg(long, value_name = "FORMAT")]
    pub output_format: Option<String>,
    /// Print only the summary by default
    #[arg(long)]
    pub summary_only: bool,

    /// Enable test generation in this directory
    #[arg(long = "generate-tests", value_name = "DIR")]
    pub generate_tests: Option<String>,
    /// Disable test generation even if the config enables it
    #[arg(long)]
    pub no_generate_tests: bool,
    /// Create a baseline now; optionally at this path
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = DEFAULT_BASELINE,
        value_name = "PATH"
    )]
    pub create_baseline: Option<String>,
    /// Do not create a baseline
    #[arg(long)]
    pub no_baseline: bool,
    /// Add the generated-test directory to an existing PHPUnit config
    #[arg(long)]
    pub phpunit_config: bool,
    /// Leave PHPUnit configuration untouched
    #[arg(long)]
    pub no_phpunit_config: bool,

    /// Write a GitHub Actions workflow
    #[arg(long)]
    pub ci: bool,
    /// Do not write a GitHub Actions workflow
    #[arg(long)]
    pub no_ci: bool,
    /// Workflow events, comma separated (default: pull_request)
    #[arg(long = "ci-events", value_name = "EVENT,EVENT")]
    pub ci_events: Vec<String>,
    /// Phanalist release used by the workflow (default: latest)
    #[arg(long = "ci-version", value_name = "VERSION")]
    pub ci_version: Option<String>,
    /// Include full rule descriptions in SARIF messages (default)
    #[arg(long = "include-rule-descriptions")]
    pub include_rule_descriptions: bool,
    /// Keep SARIF messages short
    #[arg(long = "no-rule-descriptions")]
    pub no_rule_descriptions: bool,
    /// Post a PR comment with rendered rule documentation
    #[arg(long = "post-pr-comment")]
    pub post_pr_comment: bool,
    /// Do not post a PR comment (default)
    #[arg(long = "no-post-pr-comment")]
    pub no_post_pr_comment: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CiPlan {
    events: Vec<String>,
    version: String,
    include_rule_descriptions: bool,
    post_pr_comment: bool,
}

impl Default for CiPlan {
    fn default() -> Self {
        Self {
            events: vec!["pull_request".to_string()],
            version: "latest".to_string(),
            include_rule_descriptions: true,
            post_pr_comment: false,
        }
    }
}

#[derive(Debug)]
struct Plan {
    config: Config,
    preset: String,
    baseline_path: Option<String>,
    patch_phpunit: bool,
    ci: Option<CiPlan>,
}

/// Run the `init` subcommand.
pub fn run(args: &InitArgs, config_path: &Path) -> Result<(), String> {
    let interactive = io::stdin().is_terminal();
    if !interactive && !args.yes {
        return Err(
            "init needs a terminal. Re-run interactively or pass --yes to accept defaults."
                .to_string(),
        );
    }

    let rule_info = rule_catalogue();
    let mut plan = Plan::from_args(args, load_base_config(config_path, args.force)?)?;
    if interactive {
        plan.prompt_all(&rule_info)?;
    }

    validate_paths(&plan.config.paths)?;
    if let Some(path) = &plan.baseline_path {
        if Path::new(path).exists() && !args.force {
            return Err(format!(
                "Baseline {path} already exists. Use --force to overwrite it."
            ));
        }
    }

    // Build the baseline before writing config so a failed scan leaves the
    // project untouched.
    let baseline = match &plan.baseline_path {
        Some(path) => Some((path.clone(), create_baseline(&plan.config)?)),
        None => None,
    };

    write_config(config_path, &plan.config, args.force)?;
    println!("Wrote {}", config_path.display());

    if let Some((path, baseline)) = baseline {
        let path = PathBuf::from(path);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::write(&path, baseline.to_pretty_json())
            .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
        println!("Wrote baseline {}", path.display());
    }

    if plan.patch_phpunit {
        let root = project_root(config_path);
        let config = ["phpunit.xml", "phpunit.xml.dist"]
            .into_iter()
            .map(|name| root.join(name))
            .find(|path| path.is_file());
        if let Some(config) = config {
            let output_dir = plan
                .config
                .test_generation
                .as_ref()
                .map(|generation| PathBuf::from(&generation.output_dir))
                .unwrap_or_else(|| PathBuf::from(DEFAULT_TEST_DIR));
            let output_dir = if output_dir.is_absolute() {
                output_dir
            } else {
                root.join(output_dir)
            };
            match generated_tests::add_phpunit_testsuite(&config, &output_dir) {
                Ok(true) => println!("Updated {}", config.display()),
                Ok(false) => {}
                Err(error) => {
                    return Err(format!("Failed to update {}: {error}", config.display()))
                }
            }
        }
    }

    if let Some(ci) = &plan.ci {
        let path = write_workflow(config_path, &plan.config, ci, args.force)?;
        println!("Wrote workflow {}", path.display());
    }

    Ok(())
}

impl Plan {
    fn from_args(args: &InitArgs, mut config: Config) -> Result<Self, String> {
        if !args.src.is_empty() {
            config.paths = args.src.clone();
        }
        if !args.exclude.is_empty() {
            config.exclude_paths = args.exclude.clone();
        }
        if args.taint_all && args.request_only {
            return Err("--taint-all and --request-only cannot be used together".to_string());
        }
        if args.taint_all {
            config.taint_source_all = true;
        }
        if args.request_only {
            config.taint_source_all = false;
        }
        if let Some(format) = &args.output_format {
            config.output_format = parse_format(format)?;
        }
        config.summary_only = args.summary_only || config.summary_only;

        let preset = args.preset.clone().unwrap_or_else(|| "all".to_string());
        apply_preset(&mut config, &preset)?;
        apply_rule_selection(&mut config, &args.enable_rules, &args.disable_rules)?;
        for entry in &args.set {
            apply_set(&mut config, entry)?;
        }

        if let Some(dir) = &args.generate_tests {
            config.test_generation = Some(TestGenerationConfig {
                output_dir: dir.clone(),
            });
        }
        if args.no_generate_tests {
            config.test_generation = None;
        }

        let baseline_path = if args.no_baseline {
            None
        } else {
            args.create_baseline
                .clone()
                .or_else(|| config.baseline_path.clone())
        };
        config.baseline_path = baseline_path.clone();

        let patch_phpunit = if args.no_phpunit_config {
            false
        } else {
            args.phpunit_config || config.test_generation.is_some()
        };

        let wants_ci = args.ci
            || !args.ci_events.is_empty()
            || args.ci_version.is_some()
            || args.include_rule_descriptions
            || args.no_rule_descriptions
            || args.post_pr_comment
            || args.no_post_pr_comment;
        let mut ci = if wants_ci {
            Some(CiPlan::default())
        } else {
            None
        };
        if args.no_ci {
            ci = None;
        }
        if args.include_rule_descriptions && args.no_rule_descriptions {
            return Err(
                "--include-rule-descriptions and --no-rule-descriptions cannot be used together"
                    .to_string(),
            );
        }
        if args.post_pr_comment && args.no_post_pr_comment {
            return Err(
                "--post-pr-comment and --no-post-pr-comment cannot be used together".to_string(),
            );
        }
        if let Some(plan) = ci.as_mut() {
            if !args.ci_events.is_empty() {
                let mut events = Vec::new();
                for entry in &args.ci_events {
                    for event in entry.split(',') {
                        let event = event.trim();
                        validate_event(event)?;
                        if !events.iter().any(|existing| existing == event) {
                            events.push(event.to_string());
                        }
                    }
                }
                plan.events = events;
            }
            if let Some(version) = &args.ci_version {
                validate_version(version)?;
                plan.version = version.clone();
            }
            if args.include_rule_descriptions {
                plan.include_rule_descriptions = true;
            }
            if args.no_rule_descriptions {
                plan.include_rule_descriptions = false;
            }
            if args.post_pr_comment {
                plan.post_pr_comment = true;
            }
            if args.no_post_pr_comment {
                plan.post_pr_comment = false;
            }
        }

        Ok(Self {
            config,
            preset,
            baseline_path,
            patch_phpunit,
            ci,
        })
    }

    fn prompt_all(&mut self, rule_info: &BTreeMap<String, String>) -> Result<(), String> {
        println!("\nPhanalist setup — press Enter to accept the value in brackets.\n");

        println!("Project layout");
        self.config.paths = prompt_list("Scan paths", &self.config.paths)?;
        self.config.exclude_paths = prompt_list("Exclude paths/globs", &self.config.exclude_paths)?;
        validate_paths(&self.config.paths)?;

        println!("\nRules");
        self.prompt_rules(rule_info)?;
        self.prompt_rule_settings()?;

        println!("\nTaint analysis");
        self.config.taint_source_all = prompt_bool(
            "Report all tainted findings, including parameter-derived leads?",
            self.config.taint_source_all,
        )?;

        println!("\nOutput");
        let format = prompt_choice(
            "Default output format",
            &["text", "json", "sarif", "codeclimate"],
            self.config.output_format.as_str(),
        )?;
        self.config.output_format = parse_format(&format)?;
        self.config.summary_only =
            prompt_bool("Print only a summary by default?", self.config.summary_only)?;

        println!("\nGenerated regression tests");
        let generate = prompt_bool(
            "Generate PHPUnit tests from Z3 counterexamples?",
            self.config.test_generation.is_some(),
        )?;
        if generate {
            let current = self
                .config
                .test_generation
                .as_ref()
                .map(|generation| generation.output_dir.as_str())
                .unwrap_or(DEFAULT_TEST_DIR);
            let output_dir = prompt_line("Test output directory", current)?;
            self.config.test_generation = Some(TestGenerationConfig {
                output_dir: output_dir.clone(),
            });
            self.patch_phpunit = prompt_bool(
                "Add the generated directory to an existing PHPUnit config?",
                self.patch_phpunit,
            )?;
        } else {
            self.config.test_generation = None;
            self.patch_phpunit = false;
        }

        println!("\nBaseline");
        let baseline = prompt_bool(
            "Create a baseline from the current code?",
            self.baseline_path.is_some(),
        )?;
        if baseline {
            let current = self.baseline_path.as_deref().unwrap_or(DEFAULT_BASELINE);
            self.baseline_path = Some(prompt_line("Baseline path", current)?);
            self.config.baseline_path = self.baseline_path.clone();
        } else {
            self.baseline_path = None;
            self.config.baseline_path = None;
        }

        println!("\nCI");
        let ci = prompt_bool("Write a GitHub Actions workflow?", self.ci.is_some())?;
        if ci {
            let mut plan = self.ci.clone().unwrap_or_default();
            plan.events = prompt_list("Workflow events", &plan.events)?;
            if plan.events.is_empty() {
                plan.events.push("pull_request".to_string());
            }
            for event in &plan.events {
                validate_event(event)?;
            }
            plan.version = prompt_line("Phanalist release", &plan.version)?;
            validate_version(&plan.version)?;
            plan.include_rule_descriptions = prompt_bool(
                "Include full rule descriptions in SARIF messages?",
                plan.include_rule_descriptions,
            )?;
            plan.post_pr_comment = prompt_bool(
                "Post a PR comment with rendered rule documentation?",
                plan.post_pr_comment,
            )?;
            self.ci = Some(plan);
        } else {
            self.ci = None;
        }

        Ok(())
    }

    fn prompt_rules(&mut self, rule_info: &BTreeMap<String, String>) -> Result<(), String> {
        let preset = prompt_choice(
            "Preset",
            &["all", "minimal", "verification", "resource", "custom"],
            &self.preset,
        )?;
        if preset != "custom" {
            apply_preset(&mut self.config, &preset)?;
            self.preset = preset.to_string();
        }

        let groups = [
            ("E0000-E0049: structure and verification", 0..50),
            ("E0050-E0099: security rules", 50..100),
            ("E0100+: extended security rules", 100..200),
        ];
        for (label, range) in groups {
            let codes: Vec<String> = rule_info
                .keys()
                .filter(|code| {
                    code.trim_start_matches('E')
                        .parse::<u32>()
                        .is_ok_and(|number| range.contains(&number))
                })
                .cloned()
                .collect();
            if codes.is_empty() {
                continue;
            }
            let all_enabled = codes.iter().all(|code| is_rule_enabled(&self.config, code));
            if prompt_bool(
                &format!("Enable {} ({} rules)?", label, codes.len()),
                all_enabled,
            )? {
                for code in &codes {
                    set_rule_enabled(&mut self.config, code, true);
                }
            } else {
                println!("  Select rules individually:");
                for code in codes {
                    let description = rule_info.get(&code).map(String::as_str).unwrap_or("");
                    let question = if description.is_empty() {
                        format!("  {code}?")
                    } else {
                        format!("  {code} — {description}?")
                    };
                    let enabled = prompt_bool(&question, is_rule_enabled(&self.config, &code))?;
                    set_rule_enabled(&mut self.config, &code, enabled);
                }
            }
        }
        Ok(())
    }

    fn prompt_rule_settings(&mut self) -> Result<(), String> {
        let verification: BTreeSet<String> = FindingClass::ALL
            .iter()
            .map(|class| class.code().to_string())
            .collect();
        let resource: BTreeSet<String> = ResourceClass::all()
            .iter()
            .map(|class| class.code().to_string())
            .collect();

        println!("\nRule settings");
        let mut asked = HashSet::new();
        let codes: Vec<String> = self.config.rules.keys().cloned().collect();
        for code in codes {
            let group = if verification.contains(&code) {
                Some(("verification", &verification))
            } else if resource.contains(&code) {
                Some(("resource", &resource))
            } else {
                None
            };

            if let Some((label, members)) = group {
                if !asked.insert(label) {
                    continue;
                }
                let shared = members
                    .iter()
                    .filter_map(|member| self.config.rules.get(member))
                    .all(|value| self.config.rules.get(&code) == Some(value));
                if shared {
                    let current = self.config.rules.get(&code).cloned().unwrap_or_default();
                    let updated = prompt_json_object(&format!("{label} settings"), &current)?;
                    for member in members {
                        self.config.rules.insert(member.clone(), updated.clone());
                    }
                    continue;
                }
            }

            let current = self.config.rules.get(&code).cloned().unwrap_or_default();
            let updated = prompt_json_object(&format!("{code} settings"), &current)?;
            self.config.rules.insert(code, updated);
        }
        Ok(())
    }
}

fn load_base_config(path: &Path, force: bool) -> Result<Config, String> {
    if !path.exists() {
        return Ok(Config::default());
    }
    if !force {
        return Err(format!(
            "{} already exists. Use --force to overwrite it.",
            path.display()
        ));
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    Config::from_yaml_str(&text)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))
}

fn rule_catalogue() -> BTreeMap<String, String> {
    rules::all_rules()
        .into_iter()
        .map(|(code, rule)| (code, rule.description()))
        .collect()
}

fn validate_paths(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("At least one scan path is required".to_string());
    }
    for path in paths {
        if !Path::new(path).exists() {
            return Err(format!("Scan path {path} does not exist"));
        }
    }
    Ok(())
}

fn apply_preset(config: &mut Config, preset: &str) -> Result<(), String> {
    let codes: Vec<String> = match preset {
        "all" | "recommended" => vec![],
        "minimal" => vec![
            "E0031", "E0032", "E0033", "E0036", "E0037", "E0038", "E0039",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        "verification" => FindingClass::ALL
            .iter()
            .map(|class| class.code().to_string())
            .collect(),
        "resource" => ResourceClass::all()
            .iter()
            .map(|class| class.code().to_string())
            .collect(),
        "custom" => return Ok(()),
        other => {
            return Err(format!(
                "Unknown preset {other}. Choose all, minimal, verification, resource, or custom"
            ))
        }
    };
    config.enabled_rules = codes;
    config.disable_rules.clear();
    Ok(())
}

fn is_rule_enabled(config: &Config, code: &str) -> bool {
    let enabled = config.enabled_rules.is_empty() || config.enabled_rules.iter().any(|c| c == code);
    enabled && !config.disable_rules.iter().any(|c| c == code)
}

fn set_rule_enabled(config: &mut Config, code: &str, enabled: bool) {
    config.disable_rules.retain(|existing| existing != code);
    if enabled {
        // An empty enable list means "all rules", so adding one code would
        // accidentally narrow the selection. Only record explicit additions
        // when a preset already narrowed it.
        if !config.enabled_rules.is_empty() && !config.enabled_rules.iter().any(|c| c == code) {
            config.enabled_rules.push(code.to_string());
        }
    } else {
        config.enabled_rules.retain(|existing| existing != code);
        config.disable_rules.push(code.to_string());
    }
}

fn apply_rule_selection(
    config: &mut Config,
    enable: &[String],
    disable: &[String],
) -> Result<(), String> {
    let known = rule_catalogue();
    for code in enable.iter().chain(disable.iter()) {
        if !known.contains_key(code) {
            return Err(format!("Unknown rule code {code}"));
        }
    }
    for code in enable {
        set_rule_enabled(config, code, true);
    }
    for code in disable {
        set_rule_enabled(config, code, false);
    }
    Ok(())
}

fn apply_set(config: &mut Config, entry: &str) -> Result<(), String> {
    let (assignment, raw_value) = entry
        .split_once('=')
        .ok_or_else(|| format!("Invalid --set {entry}; expected RULE.KEY=VALUE"))?;
    let (code, key) = assignment
        .rsplit_once('.')
        .ok_or_else(|| format!("Invalid --set {entry}; expected RULE.KEY=VALUE"))?;
    let Some(settings) = config.rules.get_mut(code) else {
        return Err(format!("Unknown rule code {code} in --set {entry}"));
    };
    let Some(current) = settings.get(key) else {
        return Err(format!("Unknown setting {code}.{key} in --set {entry}"));
    };
    let value = parse_json_like(current, raw_value)?;
    settings
        .as_object_mut()
        .ok_or_else(|| format!("Settings for {code} are not an object"))?
        .insert(key.to_string(), value);
    Ok(())
}

fn parse_format(value: &str) -> Result<Format, String> {
    Format::from_str(value).map_err(|_| {
        format!("Unknown output format {value}; choose text, json, sarif, or codeclimate")
    })
}

fn validate_event(event: &str) -> Result<(), String> {
    let valid = !event.is_empty()
        && event
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    if valid {
        Ok(())
    } else {
        Err(format!("Invalid GitHub event name {event:?}"))
    }
}

fn validate_version(version: &str) -> Result<(), String> {
    let valid = !version.is_empty()
        && version.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        });
    if valid {
        Ok(())
    } else {
        Err(format!("Invalid Phanalist version {version:?}"))
    }
}

fn create_baseline(config: &Config) -> Result<Baseline, String> {
    let scan_config = Config {
        test_generation: None,
        ..config.clone()
    };
    let analyze = Analyse::new(&scan_config);
    analyze.prepare_project(&scan_config.paths, &scan_config, 0, None);

    let mut aggregate = Results::default();
    for path in &scan_config.paths {
        aggregate.files.extend(
            analyze
                .scan_prepared(
                    path.clone(),
                    &scan_config,
                    false,
                    &Format::json,
                    0,
                    false,
                    None,
                )
                .files,
        );
    }
    Ok(Baseline::from_results(&aggregate))
}

fn write_config(path: &Path, config: &Config, force: bool) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    if path.exists() {
        if !force {
            return Err(format!(
                "{} already exists. Use --force to overwrite it.",
                path.display()
            ));
        }
        let backup = path.with_extension(format!(
            "{}.bak",
            path.extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("yaml")
        ));
        fs::copy(path, &backup)
            .map_err(|error| format!("Failed to back up to {}: {error}", backup.display()))?;
    }
    config
        .save_to(path)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

fn project_root(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn write_workflow(
    config_path: &Path,
    config: &Config,
    ci: &CiPlan,
    force: bool,
) -> Result<PathBuf, String> {
    let root = project_root(config_path);
    let path = root.join(".github/workflows/phanalist.yml");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    if path.exists() && !force {
        return Ok(path);
    }

    let mut events = String::new();
    for event in &ci.events {
        let _ = writeln!(events, "  {event}:");
    }

    // The action always reads `${GITHUB_WORKSPACE}/phanalist.yaml`, so point it
    // at a custom config name by copying it before the action runs.
    let relative_config = relative_to_root(&root, config_path);
    let config_step = if relative_config == Path::new("phanalist.yaml") {
        String::new()
    } else {
        format!(
            "\n      - name: Provide phanalist.yaml\n        run: cp {} phanalist.yaml\n",
            yaml_string(&relative_config.to_string_lossy())
        )
    };

    // The action accepts a single source path. Configs with more paths can be
    // narrowed here; scanning the workspace root is the safe fallback.
    let src = if config.paths.len() == 1 {
        config.paths[0].as_str()
    } else {
        "."
    };

    let content = format!(
        r#"name: Phanalist
on:
{events}
permissions:
  contents: read
  security-events: write

jobs:
  phanalist:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
{config_step}      - name: Phanalist
        uses: denzyldick/phanalist-action@v1
        with:
          src: {src}
          version: {version}
          include_rule_descriptions: {include_descriptions}
          post_pr_comment: {post_comment}
"#,
        events = events,
        config_step = config_step,
        src = yaml_string(src),
        version = yaml_string(&ci.version),
        include_descriptions = yaml_string(&ci.include_rule_descriptions.to_string()),
        post_comment = yaml_string(&ci.post_pr_comment.to_string()),
    );

    fs::write(&path, content)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
    Ok(path)
}

fn relative_to_root(root: &Path, path: &Path) -> PathBuf {
    let absolutize = |value: &Path| {
        if value.is_absolute() {
            value.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(value)
        }
    };
    let root = absolutize(root);
    let path = absolutize(path);
    let common = root
        .components()
        .zip(path.components())
        .take_while(|(left, right)| left == right)
        .count();
    let mut out = PathBuf::new();
    for _ in common..root.components().count() {
        out.push("..");
    }
    for component in path.components().skip(common) {
        out.push(component.as_os_str());
    }
    out
}

fn yaml_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn prompt_line(question: &str, default: &str) -> Result<String, String> {
    print!("{question} [{default}]: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("Failed to write prompt: {error}"))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("Failed to read answer: {error}"))?;
    let answer = line.trim();
    Ok(if answer.is_empty() {
        default.to_string()
    } else {
        answer.to_string()
    })
}

fn prompt_bool(question: &str, default: bool) -> Result<bool, String> {
    let hint = if default { "Y/n" } else { "y/N" };
    loop {
        let answer = prompt_line(
            &format!("{question} ({hint})"),
            if default { "y" } else { "n" },
        )?;
        match answer.to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please answer y or n."),
        }
    }
}

fn prompt_choice(question: &str, choices: &[&str], default: &str) -> Result<String, String> {
    loop {
        let answer = prompt_line(&format!("{question} ({})", choices.join(", ")), default)?;
        if choices.contains(&answer.as_str()) {
            return Ok(answer);
        }
        println!("Choose one of: {}", choices.join(", "));
    }
}

fn prompt_list(question: &str, current: &[String]) -> Result<Vec<String>, String> {
    let default = current.join(", ");
    let answer = prompt_line(question, &default)?;
    Ok(answer
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect())
}

fn prompt_json_object(question: &str, current: &JsonValue) -> Result<JsonValue, String> {
    println!("\n{question}");
    let Some(object) = current.as_object() else {
        let default = serde_json::to_string(current).map_err(|error| error.to_string())?;
        let answer = prompt_line("JSON value", &default)?;
        return serde_json::from_str(&answer)
            .map_err(|error| format!("Invalid JSON value: {error}"));
    };

    let mut updated = object.clone();
    let mut keys: Vec<&String> = object.keys().collect();
    keys.sort();
    for key in keys {
        let value = &object[key];
        let new_value = prompt_json_value(key, value)?;
        updated.insert(key.clone(), new_value);
    }
    Ok(JsonValue::Object(updated))
}

fn prompt_json_value(label: &str, current: &JsonValue) -> Result<JsonValue, String> {
    let default = json_default(current);
    let answer = prompt_line(label, &default)?;
    if answer == default {
        return Ok(current.clone());
    }
    parse_json_like(current, &answer)
}

fn json_default(value: &JsonValue) -> String {
    match value {
        JsonValue::String(text) => text.clone(),
        JsonValue::Bool(flag) => flag.to_string(),
        JsonValue::Number(number) => number.to_string(),
        JsonValue::Null => "null".to_string(),
        JsonValue::Array(items) => items
            .iter()
            .map(|item| match item {
                JsonValue::String(text) => text.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        JsonValue::Object(_) => value.to_string(),
    }
}

fn parse_json_like(current: &JsonValue, text: &str) -> Result<JsonValue, String> {
    match current {
        JsonValue::String(_) => Ok(JsonValue::String(text.to_string())),
        JsonValue::Bool(_) => {
            let flag = match text.to_ascii_lowercase().as_str() {
                "true" | "y" | "yes" | "1" => true,
                "false" | "n" | "no" | "0" => false,
                other => return Err(format!("Invalid boolean {other:?}")),
            };
            Ok(JsonValue::Bool(flag))
        }
        JsonValue::Number(number) if number.is_u64() => text
            .parse::<u64>()
            .map(JsonValue::from)
            .map_err(|error| format!("Invalid unsigned integer: {error}")),
        JsonValue::Number(number) if number.is_i64() => text
            .parse::<i64>()
            .map(JsonValue::from)
            .map_err(|error| format!("Invalid integer: {error}")),
        JsonValue::Number(_) => {
            let value = text
                .parse::<f64>()
                .map_err(|error| format!("Invalid number: {error}"))?;
            serde_json::Number::from_f64(value)
                .map(JsonValue::Number)
                .ok_or_else(|| "Invalid number: value is not finite".to_string())
        }
        JsonValue::Array(items) if items.iter().all(JsonValue::is_string) => Ok(JsonValue::Array(
            text.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| JsonValue::String(item.to_string()))
                .collect(),
        )),
        JsonValue::Null => {
            if text == "null" {
                Ok(JsonValue::Null)
            } else {
                serde_json::from_str(text).map_err(|error| format!("Invalid JSON value: {error}"))
            }
        }
        _ => serde_json::from_str(text).map_err(|error| format!("Invalid JSON value: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_use_enable_lists() {
        let mut config = Config::default();
        apply_preset(&mut config, "verification").unwrap();
        assert_eq!(config.enabled_rules.len(), FindingClass::ALL.len());
        assert!(config.disable_rules.is_empty());

        apply_preset(&mut config, "all").unwrap();
        assert!(config.enabled_rules.is_empty());
    }

    #[test]
    fn rule_selection_wins_over_preset() {
        let mut config = Config::default();
        apply_preset(&mut config, "all").unwrap();
        apply_rule_selection(&mut config, &[], &["E0031".to_string()]).unwrap();
        assert!(!is_rule_enabled(&config, "E0031"));
        assert!(is_rule_enabled(&config, "E0032"));
    }

    #[test]
    fn set_updates_the_existing_json_type() {
        let mut config = Config::default();
        apply_set(&mut config, "E0031.timeout_ms=2000").unwrap();
        assert_eq!(config.rules["E0031"]["timeout_ms"], serde_json::json!(2000));
        assert!(apply_set(&mut config, "E0031.unknown_setting=1").is_err());
    }

    #[test]
    fn workflow_uses_only_the_action_inputs() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("phanalist-init-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let config_path = root.join("phanalist.yaml");
        let ci = CiPlan::default();
        let config = Config::default();
        let path = write_workflow(&config_path, &config, &ci, true).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("  pull_request:\n"));
        assert!(content.contains("uses: denzyldick/phanalist-action@v1"));
        assert!(content.contains("src: './src'"));
        assert!(content.contains("version: 'latest'"));
        assert!(content.contains("include_rule_descriptions: 'true'"));
        assert!(content.contains("post_pr_comment: 'false'"));
        assert!(!content.contains("upload-sarif"));
        assert!(!content.contains("curl"));
        fs::remove_dir_all(&root).unwrap();
    }
}
