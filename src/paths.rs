//! Shared path helpers: normalize scanned paths to a stable, portable form and
//! match them against `exclude_paths` patterns.

use std::path::Path;

/// Normalize a path to a stable relative form: relative to the current working
/// directory, posix separators, no leading `./`. Shared by `exclude_paths`
/// matching (and, later, baseline keys) so every comparison agrees on shape.
pub fn normalize_relative(path: &Path) -> String {
    let rel = match std::env::current_dir() {
        Ok(cwd) => path.strip_prefix(&cwd).unwrap_or(path),
        Err(_) => path,
    };
    let s = rel.to_string_lossy().replace('\\', "/");
    s.strip_prefix("./").unwrap_or(&s).to_string()
}

/// True if `relative_path` (already normalized) is excluded by any pattern.
///
/// A pattern matches when it is a path-segment prefix of the file
/// (`var/cache` excludes `var/cache/x.php`) or a glob match (`**/*.generated.php`).
pub fn is_excluded(relative_path: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        let pattern = normalize_pattern(pattern);
        if pattern.is_empty() {
            return false;
        }
        if pattern.contains('*') || pattern.contains('?') {
            glob_match(&pattern, relative_path)
        } else {
            relative_path == pattern
                || relative_path.starts_with(&format!("{pattern}/"))
        }
    })
}

/// True if the pattern uses glob syntax (`*`, `**`, `?`) rather than a plain
/// directory/path prefix.
pub fn is_glob(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

/// Literal (non-glob) exclude patterns whose path does not exist on disk,
/// returned verbatim for a "probably a typo" warning. Globs are skipped: they
/// match whatever is present, so matching nothing is not an error.
pub fn missing_literal_excludes(patterns: &[String]) -> Vec<String> {
    patterns
        .iter()
        .filter(|p| !is_glob(p))
        .filter(|p| {
            let normalized = normalize_pattern(p);
            !normalized.is_empty() && !Path::new(&normalized).exists()
        })
        .cloned()
        .collect()
}

/// Bring a pattern to the same shape as a normalized path: posix separators, no
/// leading `./`, no trailing `/`.
fn normalize_pattern(pattern: &str) -> String {
    let p = pattern.replace('\\', "/");
    let p = p.strip_prefix("./").unwrap_or(&p);
    p.trim_end_matches('/').to_string()
}

/// Glob tokens. `*` matches within a path segment, `**` (and an optional
/// following `/`) crosses segment boundaries.
enum Token {
    /// `**/` — zero or more whole path segments.
    DoubleStarSlash,
    /// `**` — any run of characters, including `/`.
    DoubleStar,
    /// `*` — any run of characters except `/`.
    Star,
    /// `?` — a single character except `/`.
    Question,
    Char(char),
}

fn tokenize(pattern: &str) -> Vec<Token> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                if i + 2 < chars.len() && chars[i + 2] == '/' {
                    tokens.push(Token::DoubleStarSlash);
                    i += 3;
                } else {
                    tokens.push(Token::DoubleStar);
                    i += 2;
                }
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '?' => {
                tokens.push(Token::Question);
                i += 1;
            }
            c => {
                tokens.push(Token::Char(c));
                i += 1;
            }
        }
    }
    tokens
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let tokens = tokenize(pattern);
    let text: Vec<char> = text.chars().collect();
    matches_from(&tokens, &text)
}

fn matches_from(tokens: &[Token], text: &[char]) -> bool {
    let Some((head, rest)) = tokens.split_first() else {
        return text.is_empty();
    };

    match head {
        Token::DoubleStarSlash => {
            // Zero segments: rest must match the text as-is.
            if matches_from(rest, text) {
                return true;
            }
            // One or more segments: consume up to and including each `/`.
            for (i, c) in text.iter().enumerate() {
                if *c == '/' && matches_from(rest, &text[i + 1..]) {
                    return true;
                }
            }
            false
        }
        Token::DoubleStar => (0..=text.len()).any(|i| matches_from(rest, &text[i..])),
        Token::Star => {
            for i in 0..=text.len() {
                if i > 0 && text[i - 1] == '/' {
                    break;
                }
                if matches_from(rest, &text[i..]) {
                    return true;
                }
            }
            false
        }
        Token::Question => {
            !text.is_empty() && text[0] != '/' && matches_from(rest, &text[1..])
        }
        Token::Char(c) => {
            !text.is_empty() && text[0] == *c && matches_from(rest, &text[1..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn normalize_strips_leading_dot_slash() {
        assert_eq!(normalize_relative(Path::new("./src/Foo.php")), "src/Foo.php");
    }

    #[test]
    fn normalize_converts_backslashes() {
        assert_eq!(normalize_relative(Path::new("src\\Foo.php")), "src/Foo.php");
    }

    #[test]
    fn normalize_makes_absolute_path_cwd_relative() {
        let abs = std::env::current_dir().unwrap().join("src/Foo.php");
        assert_eq!(normalize_relative(&abs), "src/Foo.php");
    }

    #[test]
    fn normalize_plain_relative_is_unchanged() {
        assert_eq!(normalize_relative(&PathBuf::from("src/Foo.php")), "src/Foo.php");
    }

    #[test]
    fn excludes_directory_prefix() {
        assert!(is_excluded("var/cache/x.php", &["var/cache".to_string()]));
    }

    #[test]
    fn excludes_exact_directory() {
        assert!(is_excluded("var/cache", &["var/cache".to_string()]));
    }

    #[test]
    fn prefix_respects_segment_boundary() {
        assert!(!is_excluded("var/cachefoo.php", &["var/cache".to_string()]));
    }

    #[test]
    fn does_not_exclude_unrelated_path() {
        assert!(!is_excluded("src/Foo.php", &["var/cache".to_string()]));
    }

    #[test]
    fn pattern_leading_dot_slash_is_normalized() {
        assert!(is_excluded("var/cache/x.php", &["./var/cache".to_string()]));
    }

    #[test]
    fn glob_double_star_suffix_matches_any_depth() {
        let p = ["**/*.generated.php".to_string()];
        assert!(is_excluded("Foo.generated.php", &p));
        assert!(is_excluded("a/b/Foo.generated.php", &p));
    }

    #[test]
    fn single_star_does_not_cross_slash() {
        let p = ["*.php".to_string()];
        assert!(is_excluded("b.php", &p));
        assert!(!is_excluded("a/b.php", &p));
    }

    #[test]
    fn empty_patterns_exclude_nothing() {
        assert!(!is_excluded("anything.php", &[]));
    }

    #[test]
    fn is_glob_detects_wildcards() {
        assert!(is_glob("**/*.php"));
        assert!(is_glob("*.php"));
        assert!(is_glob("a?b"));
    }

    #[test]
    fn is_glob_false_for_plain_paths() {
        assert!(!is_glob("var/cache"));
        assert!(!is_glob("database/migrations"));
    }

    #[test]
    fn missing_literal_excludes_flags_nonexistent_dirs() {
        // The crate root (cwd during tests) has `src/` but not this name.
        let patterns = vec![
            "src".to_string(),
            "no_such_dir_phanalist_xyz".to_string(),
        ];
        assert_eq!(
            missing_literal_excludes(&patterns),
            vec!["no_such_dir_phanalist_xyz".to_string()]
        );
    }

    #[test]
    fn missing_literal_excludes_skips_globs() {
        let patterns = vec!["**/no_such_thing_*.php".to_string()];
        assert!(missing_literal_excludes(&patterns).is_empty());
    }

    #[test]
    fn missing_literal_excludes_ignores_existing_and_dot_slash() {
        let patterns = vec!["./src".to_string()];
        assert!(missing_literal_excludes(&patterns).is_empty());
    }
}
