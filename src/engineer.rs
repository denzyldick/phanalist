use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bumpalo::Bump;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Utc};
use git2::Repository;
use gix;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};

use crate::analyse::Analyse;
use crate::config::Config;
use crate::file::File;
use crate::outputs::Format;
use crate::results::{EngineerEntry, EngineerReport, RuleChange, Violation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameConfig {
    pub since: Option<String>,
    pub until: Option<String>,
    pub exclude_authors: Vec<String>,
    pub min_violations: u64,
}

impl Default for BlameConfig {
    fn default() -> Self {
        Self {
            since: None,
            until: None,
            exclude_authors: vec![],
            min_violations: 0,
        }
    }
}

/// Parse a human-readable or absolute date string into a timestamp.
/// Supported formats:
///   - RFC 3339: "2025-01-01T00:00:00Z"
///   - ISO 8601 date: "2025-01-01"
///   - Relative: "30 days", "30 days ago", "1 year", "3 months", "7d", "1y"
pub fn parse_relative_date(s: &str) -> Option<DateTime<FixedOffset>> {
    let now = Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap());
    let s = s.trim();

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt);
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_local_timezone(FixedOffset::east_opt(0).unwrap()).unwrap());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.and_time(NaiveTime::MIN).and_local_timezone(FixedOffset::east_opt(0).unwrap()).unwrap());
    }

    // Handle compact formats: "7d", "1y", "3mo"
    let s_lower = s.to_lowercase();
    if s_lower.ends_with('d') && s_lower.len() > 1 {
        if let Ok(n) = s_lower[..s_lower.len() - 1].parse::<i64>() {
            return Some(now - TimeDelta::try_days(n).unwrap());
        }
    }
    if s_lower.ends_with('y') && s_lower.len() > 1 {
        if let Ok(n) = s_lower[..s_lower.len() - 1].parse::<i64>() {
            return Some(now - TimeDelta::try_days(n * 365).unwrap());
        }
    }

    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 2 && (parts[1] == "day" || parts[1] == "days" || parts[1] == "d") {
        let n: i64 = parts[0].parse().ok()?;
        return Some(now - TimeDelta::try_days(n).unwrap());
    }
    if parts.len() == 2 && (parts[1] == "month" || parts[1] == "months") {
        let n: i64 = parts[0].parse().ok()?;
        return Some(now - TimeDelta::try_days(n * 30).unwrap());
    }
    if parts.len() == 2 && (parts[1] == "year" || parts[1] == "years" || parts[1] == "y") {
        let n: i64 = parts[0].parse().ok()?;
        return Some(now - TimeDelta::try_days(n * 365).unwrap());
    }
    if parts.len() == 3 && parts[1] == "days" && parts[2] == "ago" {
        let n: i64 = parts[0].parse().ok()?;
        return Some(now - TimeDelta::try_days(n).unwrap());
    }

    None
}

pub fn is_author_excluded(name: &str, email: &str, exclude: &[String]) -> bool {
    let name_lower = name.to_lowercase();
    let email_lower = email.to_lowercase();
    exclude.iter().any(|e| {
        let e_lower = e.to_lowercase();
        name_lower.contains(&e_lower) || email_lower.contains(&e_lower)
    })
}

pub struct EngineerBlame {
    repo: Mutex<Repository>,
    repo_path: PathBuf,
    gix_repo: gix::Repository,
    since_ts: Option<i64>,
    until_ts: Option<i64>,
    exclude_authors: Vec<String>,
    min_violations: u64,
    blame_cache: Arc<Mutex<HashMap<String, Arc<Vec<BlameLine>>>>>,
}

#[derive(Clone)]
struct BlameLine {
    author: String,
    #[allow(dead_code)]
    author_email: String,
    time: i64,
}

impl EngineerBlame {
    pub fn new(
        repo_path: &Path,
        config: &BlameConfig,
    ) -> Result<Self, String> {
        let repo = Repository::open(repo_path).map_err(|e| format!("Cannot open git repo: {e}"))?;
        let gix_repo = gix::open(repo_path)
            .map_err(|e| format!("Cannot open gix repo: {e}"))?;

        let since_ts = config.since.as_ref().and_then(|s| {
            parse_relative_date(s).map(|dt| dt.timestamp())
        });

        let until_ts = config.until.as_ref().and_then(|s| {
            parse_relative_date(s).map(|dt| dt.timestamp())
        });

        Ok(Self {
            repo: Mutex::new(repo),
            repo_path: repo_path.to_path_buf(),
            gix_repo,
            since_ts,
            until_ts,
            exclude_authors: config.exclude_authors.clone(),
            min_violations: config.min_violations,
            blame_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn blame_file_with_repo(&self, rel_path: &str, gix_repo: &gix::Repository) -> Result<Arc<Vec<BlameLine>>, String> {
        {
            let cache = self.blame_cache.lock().unwrap();
            if let Some(cached) = cache.get(rel_path) {
                return Ok(Arc::clone(cached));
            }
        }

        let normalized = rel_path.strip_prefix("./").unwrap_or(rel_path);
        let workdir = self.repo.lock().unwrap().workdir().unwrap_or(Path::new(".")).to_path_buf();
        let path = Path::new(normalized)
            .strip_prefix(&workdir)
            .unwrap_or(Path::new(normalized));

        let head = gix_repo.head_id()
            .map_err(|e| format!("Cannot get HEAD: {e}"))?;
        let path_str = path.to_string_lossy();
        let outcome = gix_repo.blame_file(
            path_str.as_bytes().into(),
            head,
            Default::default(),
        ).map_err(|e| format!("Cannot blame {}: {e}", rel_path))?;

        let mut lines = Vec::new();
        for entry in outcome.entries {
            let commit = gix_repo.find_commit(entry.commit_id)
                .map_err(|e| format!("Cannot find commit {}: {e}", entry.commit_id))?;
            let (name, email, time) = match commit.author() {
                Ok(sig) => (
                    sig.name.to_string(),
                    sig.email.to_string(),
                    sig.time().ok().map(|t| t.seconds).unwrap_or(0),
                ),
                Err(_) => ("unknown".to_string(), String::new(), 0),
            };
            let count = entry.len.get() as usize;
            for _ in 0..count {
                lines.push(BlameLine {
                    author: name.clone(),
                    author_email: email.clone(),
                    time,
                });
            }
        }

        let result = Arc::new(lines);
        let mut cache = self.blame_cache.lock().unwrap();
        cache.insert(rel_path.to_string(), Arc::clone(&result));
        Ok(result)
    }

    fn get_file_content_at_date(&self, rel_path: &str, since_ts: i64) -> Option<String> {
        let normalized = rel_path.strip_prefix("./").unwrap_or(rel_path);
        let repo = self.repo.lock().unwrap();
        let workdir = repo.workdir().unwrap_or(Path::new(".")).to_path_buf();
        let path = Path::new(normalized)
            .strip_prefix(&workdir)
            .unwrap_or(Path::new(normalized));
        let mut revwalk = repo.revwalk().ok()?;
        revwalk.push_head().ok()?;
        revwalk.set_sorting(git2::Sort::TIME).ok()?;

        let mut best_oid = None;

        for oid_result in revwalk {
            let oid = oid_result.ok()?;
            let commit = repo.find_commit(oid).ok()?;
            let commit_time = commit.time().seconds();

            if commit_time > since_ts {
                continue;
            }

            let tree = commit.tree().ok()?;
            if tree.get_path(path).is_ok() {
                best_oid = Some(oid);
                break;
            }
        }

        let oid = best_oid?;
        let commit = repo.find_commit(oid).ok()?;
        let tree = commit.tree().ok()?;
        let entry = tree.get_path(path).ok()?;
        let blob = repo.find_blob(entry.id()).ok()?;
        let content = std::str::from_utf8(blob.content()).ok()?;
        Some(content.to_string())
    }

    /// Walk the revwalk once to find the boundary commit (first commit ≤ since_ts),
    /// then tree-diff it against HEAD to get all changed files. Returns `None`
    /// when no boundary commit exists (process everything).
    fn get_changed_files_since(&self, since_ts: i64) -> Option<HashSet<String>> {
        let repo = self.repo.lock().ok()?;
        let head = repo.head().ok()?;
        let head_oid = head.target()?;
        let head_commit = repo.find_commit(head_oid).ok()?;
        let head_tree = head_commit.tree().ok()?;

        let mut revwalk = repo.revwalk().ok()?;
        revwalk.push(head_oid).ok()?;
        revwalk.set_sorting(git2::Sort::TIME).ok()?;

        let boundary_oid = 'found: {
            for oid_result in revwalk {
                let oid = oid_result.ok()?;
                if let Ok(commit) = repo.find_commit(oid) {
                    if commit.time().seconds() <= since_ts {
                        break 'found oid;
                    }
                }
            }
            return None;
        };

        let boundary_commit = repo.find_commit(boundary_oid).ok()?;
        let boundary_tree = boundary_commit.tree().ok()?;

        let diff = repo.diff_tree_to_tree(
            Some(&boundary_tree),
            Some(&head_tree),
            None,
        ).ok()?;

        let changed: HashSet<String> = diff.deltas()
            .filter_map(|d| {
                d.new_file().path()
                    .or_else(|| d.old_file().path())
                    .map(|p| p.to_string_lossy().to_string())
            })
            .collect();

        Some(changed)
    }

    fn majority_author_in_window(
        &self,
        file_path: &str,
        start_line: usize,
        end_line: usize,
    ) -> Option<String> {
        self.majority_author_in_window_with_repo(file_path, start_line, end_line, &self.gix_repo)
    }

    fn majority_author_in_window_with_repo(
        &self,
        file_path: &str,
        start_line: usize,
        end_line: usize,
        gix_repo: &gix::Repository,
    ) -> Option<String> {
        let blame_lines = match self.blame_file_with_repo(file_path, gix_repo) {
            Ok(lines) => lines,
            Err(_) => return None,
        };

        let mut counts: HashMap<String, u64> = HashMap::new();
        let start = start_line.saturating_sub(1);
        let end = end_line.min(blame_lines.len());

        for i in start..end {
            if i >= blame_lines.len() {
                break;
            }
            let bl = &blame_lines[i];
            let time_ok = match (self.since_ts, self.until_ts) {
                (Some(s), Some(u)) => bl.time >= s && bl.time <= u,
                (Some(s), None) => bl.time >= s,
                (None, Some(u)) => bl.time <= u,
                (None, None) => true,
            };
            if time_ok {
                *counts.entry(bl.author.clone()).or_insert(0) += 1;
            }
        }

        counts.into_iter().max_by_key(|(_, c)| *c).map(|(a, _)| a)
    }

    pub fn attribute_violations(
        &self,
        results: &crate::results::Results,
        analyse: &Analyse,
        _config: &Config,
        format: &Format,
        _verbose: u8,
        blame_bar: &Option<ProgressBar>,
    ) -> EngineerReport {
        let mut report: EngineerReport = HashMap::new();

        let using_external = blame_bar.is_some();
        let file_count = if self.since_ts.is_some() || self.until_ts.is_some() {
            results.files.len()
        } else {
            results.files.iter().filter(|(_, v)| !v.is_empty()).count()
        };
        let own_bar = if !using_external { make_bar(file_count, format) } else { None };
        let bar: &Option<ProgressBar> = if using_external { blame_bar } else { &own_bar };

        if self.since_ts.is_some() || self.until_ts.is_some() {
            self.attribute_with_history(results, analyse, &mut report, bar);
        } else {
            self.attribute_current(results, &mut report, bar);
        }

        if !using_external {
            if let Some(b) = &own_bar {
                b.finish_with_message("blame done");
            }
        }

        for entry in report.values_mut() {
            entry.net = (entry.total_fixed as i64) - (entry.total_introduced as i64);
        }

        if self.min_violations > 0 {
            report.retain(|_, entry| {
                entry.total_fixed + entry.total_introduced >= self.min_violations
            });
        }

        report
    }

    fn attribute_current(
        &self,
        results: &crate::results::Results,
        report: &mut EngineerReport,
        bar: &Option<ProgressBar>,
    ) {
        let file_entries: Vec<_> = results.files.iter()
            .filter(|(_, v)| !v.is_empty())
            .collect();
        if file_entries.is_empty() {
            return;
        }

        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get()).unwrap_or(4)
            .min(file_entries.len());
        let chunk_size = (file_entries.len() + num_threads - 1) / num_threads;

        let cache = Arc::clone(&self.blame_cache);
        let exclude_authors = &self.exclude_authors;
        let since_ts = self.since_ts;
        let until_ts = self.until_ts;
        let repo_path = self.repo_path.clone();
        let workdir = self.repo.lock().unwrap().workdir()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let bar_inner = bar.clone();

        let results_mutex: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

        std::thread::scope(|s| {
            for chunk in file_entries.chunks(chunk_size) {
                let chunk = chunk.to_vec();
                let cache = Arc::clone(&cache);
                let ex = exclude_authors.clone();
                let bp = bar_inner.clone();
                let rp = repo_path.clone();
                let wd = workdir.clone();
                let res = &results_mutex;

                s.spawn(move || {
                    let gix_repo = match gix::open(&rp) {
                        Ok(r) => r,
                        Err(_) => return,
                    };

                    for (file_path, violations) in &chunk {
                        for violation in *violations {
                            let blame_lines = match get_blame_lines(&*cache, &gix_repo, file_path, &wd) {
                                Ok(l) => l,
                                Err(_) => continue,
                            };

                            let mut counts: HashMap<String, u64> = HashMap::new();
                            let start = violation.start_line.saturating_sub(1);
                            let end = violation.end_line.min(blame_lines.len());

                            for i in start..end {
                                if i >= blame_lines.len() { break; }
                                let bl = &blame_lines[i];
                                let time_ok = match (since_ts, until_ts) {
                                    (Some(s), Some(u)) => bl.time >= s && bl.time <= u,
                                    (Some(s), None) => bl.time >= s,
                                    (None, Some(u)) => bl.time <= u,
                                    (None, None) => true,
                                };
                                if time_ok {
                                    *counts.entry(bl.author.clone()).or_insert(0) += 1;
                                }
                            }

                            let author = counts.into_iter()
                                .max_by_key(|&(_, c)| c)
                                .map(|(a, _)| a);
                            let author = match author {
                                Some(a) => a,
                                None => continue,
                            };

                            if is_author_excluded(&author, "", &ex) {
                                continue;
                            }

                            res.lock().unwrap().push((author, violation.rule.clone()));
                        }
                        if let Some(ref b) = bp {
                            b.inc(1);
                        }
                    }
                });
            }
        });

        for (author, rule) in results_mutex.into_inner().unwrap() {
            let entry = report.entry(author).or_insert(EngineerEntry::default());
            entry.total_introduced += 1;
            let rule_entry = entry.rules.entry(rule).or_insert(RuleChange::default());
            rule_entry.introduced += 1;
        }
    }

    fn attribute_with_history(
        &self,
        results: &crate::results::Results,
        analyse: &Analyse,
        report: &mut EngineerReport,
        bar: &Option<ProgressBar>,
    ) {
        let arena = Bump::new();
        let since_ts = self.since_ts.unwrap_or(0);

        // Single tree-diff pass to find changed files instead of per-file revwalk
        let changed_files = if since_ts > 0 {
            self.get_changed_files_since(since_ts).unwrap_or_default()
        } else {
            HashSet::new()
        };
        let has_filter = !changed_files.is_empty();

        let repo_lock = self.repo.lock().unwrap();
        let workdir = repo_lock.workdir().map(|p| p.to_path_buf());
        drop(repo_lock);

        let file_paths: Vec<String> = results.files.keys().cloned().collect();

        for file_path in &file_paths {
            // Skip files not in the changed set
            if has_filter {
                let normalized = file_path.strip_prefix("./").unwrap_or(file_path);
                let rel = match &workdir {
                    Some(wd) => Path::new(normalized).strip_prefix(wd).unwrap_or(Path::new(normalized)),
                    None => Path::new(normalized),
                };
                if !changed_files.contains(rel.to_str().unwrap_or(normalized)) {
                    if let Some(b) = bar {
                        b.inc(1);
                    }
                    continue;
                }
            }

            let current_violations = match results.files.get(file_path) {
                Some(v) => v,
                None => continue,
            };

            let old_content = self.get_file_content_at_date(file_path, since_ts);

            let old_violations: Vec<Violation> = match old_content {
                Some(content) => {
                    let mut old_file = File::new(&arena, std::path::PathBuf::from(file_path), content);
                    let (violations, _) = analyse.analyse_file(&mut old_file, false);
                    violations
                }
                None => vec![],
            };

            let current_set: HashSet<String> = current_violations.iter().map(violation_key).collect();
            let old_set: HashSet<String> = old_violations.iter().map(violation_key).collect();

            let introduced_keys: Vec<&String> = current_set.difference(&old_set).collect();
            let fixed_keys: Vec<&String> = old_set.difference(&current_set).collect();

            let current_by_key: HashMap<String, &Violation> = current_violations.iter()
                .map(|v| (violation_key(v), v))
                .collect();
            let old_by_key: HashMap<String, &Violation> = old_violations.iter()
                .map(|v| (violation_key(v), v))
                .collect();

            for key in introduced_keys {
                if let Some(violation) = current_by_key.get(key) {
                    let author = self.majority_author_in_window(file_path, violation.start_line, violation.end_line);
                    if let Some(ref author) = author {
                        if is_author_excluded(author, "", &self.exclude_authors) {
                            continue;
                        }
                        let entry = report.entry(author.clone()).or_insert(EngineerEntry::default());
                        entry.total_introduced += 1;
                        let rule_entry = entry.rules.entry(violation.rule.clone()).or_insert(RuleChange::default());
                        rule_entry.introduced += 1;
                    }
                }
            }

            for key in fixed_keys {
                if let Some(violation) = old_by_key.get(key) {
                    let author = self.majority_author_in_window(file_path, violation.start_line, violation.end_line);
                    if let Some(ref author) = author {
                        if is_author_excluded(author, "", &self.exclude_authors) {
                            continue;
                        }
                        let entry = report.entry(author.clone()).or_insert(EngineerEntry::default());
                        entry.total_fixed += 1;
                        let rule_entry = entry.rules.entry(violation.rule.clone()).or_insert(RuleChange::default());
                        rule_entry.fixed += 1;
                    }
                }
            }

            if let Some(b) = bar {
                b.inc(1);
            }
        }
    }
}

fn get_blame_lines(
    cache: &Mutex<HashMap<String, Arc<Vec<BlameLine>>>>,
    gix_repo: &gix::Repository,
    rel_path: &str,
    workdir: &Path,
) -> Result<Arc<Vec<BlameLine>>, String> {
    {
        let c = cache.lock().unwrap();
        if let Some(cached) = c.get(rel_path) {
            return Ok(Arc::clone(cached));
        }
    }

    let normalized = rel_path.strip_prefix("./").unwrap_or(rel_path);
    let path = Path::new(normalized)
        .strip_prefix(workdir)
        .unwrap_or(Path::new(normalized));
    let head = gix_repo.head_id()
        .map_err(|e| format!("Cannot get HEAD: {e}"))?;
    let path_str = path.to_string_lossy();
    let outcome = gix_repo.blame_file(
        path_str.as_bytes().into(),
        head,
        Default::default(),
    ).map_err(|e| format!("Cannot blame {}: {e}", rel_path))?;

    let mut lines = Vec::new();
    for entry in outcome.entries {
        let commit = gix_repo.find_commit(entry.commit_id)
            .map_err(|e| format!("Cannot find commit {}: {e}", entry.commit_id))?;
        let (name, email, time) = match commit.author() {
            Ok(sig) => (
                sig.name.to_string(),
                sig.email.to_string(),
                sig.time().ok().map(|t| t.seconds).unwrap_or(0),
            ),
            Err(_) => ("unknown".to_string(), String::new(), 0),
        };
        let count = entry.len.get() as usize;
        for _ in 0..count {
            lines.push(BlameLine {
                author: name.clone(),
                author_email: email.clone(),
                time,
            });
        }
    }

    let result = Arc::new(lines);
    let mut c = cache.lock().unwrap();
    c.insert(rel_path.to_string(), Arc::clone(&result));
    Ok(result)
}

fn make_bar(total: usize, format: &Format) -> Option<ProgressBar> {
    if total > 0 && format == &Format::text {
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40}] {pos}/{len} files ({eta})")
                .unwrap()
                .progress_chars("=>"),
        );
        pb.set_message("blaming");
        Some(pb)
    } else {
        None
    }
}

fn violation_key(v: &Violation) -> String {
    format!("{}:{}:{}", v.rule, v.start_line, v.message.render())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::Message;
    use chrono::{Timelike, Datelike};

    #[test]
    fn test_parse_relative_date_rfc3339() {
        let dt = parse_relative_date("2025-01-15T10:30:00Z").unwrap();
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_parse_relative_date_iso() {
        let dt = parse_relative_date("2025-01-15").unwrap();
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
        assert_eq!(dt.hour(), 0);
    }

    #[test]
    fn test_parse_relative_date_days_ago() {
        let dt = parse_relative_date("7 days ago").unwrap();
        let expected = Utc::now()
            .with_timezone(&FixedOffset::east_opt(0).unwrap())
            - TimeDelta::try_days(7).unwrap();
        assert!((dt - expected).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_relative_date_days() {
        let dt = parse_relative_date("30 days").unwrap();
        let expected = Utc::now()
            .with_timezone(&FixedOffset::east_opt(0).unwrap())
            - TimeDelta::try_days(30).unwrap();
        assert!((dt - expected).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_relative_date_short_d() {
        let dt = parse_relative_date("7d").unwrap();
        let expected = Utc::now()
            .with_timezone(&FixedOffset::east_opt(0).unwrap())
            - TimeDelta::try_days(7).unwrap();
        assert!((dt - expected).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_relative_date_years() {
        let dt = parse_relative_date("1 year").unwrap();
        let expected = Utc::now()
            .with_timezone(&FixedOffset::east_opt(0).unwrap())
            - TimeDelta::try_days(365).unwrap();
        assert!((dt - expected).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_relative_date_short_y() {
        let dt = parse_relative_date("2y").unwrap();
        let expected = Utc::now()
            .with_timezone(&FixedOffset::east_opt(0).unwrap())
            - TimeDelta::try_days(730).unwrap();
        assert!((dt - expected).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_relative_date_months() {
        let dt = parse_relative_date("6 months").unwrap();
        let expected = Utc::now()
            .with_timezone(&FixedOffset::east_opt(0).unwrap())
            - TimeDelta::try_days(180).unwrap();
        assert!((dt - expected).num_seconds().abs() < 2);
    }

    #[test]
    fn test_parse_relative_date_invalid() {
        assert!(parse_relative_date("not a date").is_none());
        assert!(parse_relative_date("").is_none());
        assert!(parse_relative_date("abc days").is_none());
    }

    #[test]
    fn test_is_author_excluded_name() {
        let exclude = vec!["dependabot".to_string(), "bot".to_string()];
        assert!(is_author_excluded("dependabot[bot]", "dependabot@github.com", &exclude));
        assert!(is_author_excluded("My Bot Account", "bot@example.com", &exclude));
        assert!(!is_author_excluded("Alice", "alice@example.com", &exclude));
    }

    #[test]
    fn test_is_author_excluded_email() {
        let exclude = vec!["dependabot".to_string()];
        assert!(is_author_excluded("dependabot[bot]", "dependabot@github.com", &exclude));
        assert!(!is_author_excluded("Alice", "alice@example.com", &exclude));
    }

    #[test]
    fn test_is_author_excluded_case_insensitive() {
        let exclude = vec!["Dependabot".to_string()];
        assert!(is_author_excluded("dependabot[bot]", "dependabot@github.com", &exclude));
        assert!(is_author_excluded("DEPENDABOT", "dependabot@github.com", &exclude));
    }

    #[test]
    fn test_is_author_excluded_empty() {
        let exclude: Vec<String> = vec![];
        assert!(!is_author_excluded("Alice", "alice@example.com", &exclude));
        assert!(!is_author_excluded("admin", "admin@test.com", &exclude));
    }

    #[test]
    fn test_violation_key_format() {
        let v = Violation {
            rule: "E001".to_string(),
            line: "".to_string(),
            message: Message::new("test-id", "Something wrong at {col}").arg("col", "5"),
            start_line: 10,
            start_column: 1,
            end_line: 12,
            end_column: 5,
        };
        let key = violation_key(&v);
        assert_eq!(key, "E001:10:Something wrong at 5");
    }

    #[test]
    fn test_blame_config_default() {
        let config = BlameConfig::default();
        assert!(config.since.is_none());
        assert!(config.until.is_none());
        assert!(config.exclude_authors.is_empty());
        assert_eq!(config.min_violations, 0);
    }

    #[test]
    fn test_engineer_entry_net_calculation() {
        let mut entry = EngineerEntry::default();
        assert_eq!(entry.net, 0);

        entry.total_fixed = 10;
        entry.total_introduced = 3;
        entry.net = (entry.total_fixed as i64) - (entry.total_introduced as i64);
        assert_eq!(entry.net, 7);

        entry.total_fixed = 2;
        entry.total_introduced = 15;
        entry.net = (entry.total_fixed as i64) - (entry.total_introduced as i64);
        assert_eq!(entry.net, -13);
    }

    #[test]
    fn test_engineer_report_serialization() {
        let mut report: EngineerReport = HashMap::new();
        let mut entry = EngineerEntry::default();
        entry.total_introduced = 5;
        entry.net = -5;
        entry.rules.insert("E001".to_string(), RuleChange { fixed: 0, introduced: 3 });
        entry.rules.insert("E002".to_string(), RuleChange { fixed: 1, introduced: 1 });
        report.insert("Alice".to_string(), entry);

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"Alice\""));
        assert!(json.contains("\"total_introduced\":5"));
        assert!(json.contains("\"net\":-5"));
        assert!(json.contains("\"E001\""));

        let deserialized: EngineerReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized["Alice"].total_introduced, 5);
    }

    #[test]
    fn test_engineer_entry_default() {
        let entry = EngineerEntry::default();
        assert_eq!(entry.total_fixed, 0);
        assert_eq!(entry.total_introduced, 0);
        assert_eq!(entry.net, 0);
        assert!(entry.rules.is_empty());
    }
}
