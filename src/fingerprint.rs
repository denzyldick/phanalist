//! Stable finding fingerprints shared by every output format.
//!
//! A fingerprint must survive a finding moving up or down a file, so it is
//! derived from the rule, the normalized path, the message id and the offending
//! source text rather than the line number. Two identical lines in one file
//! intentionally share a fingerprint: they describe the same kind of problem at
//! the same place, and the baseline already accounts for repeats by count.

/// Compute the stable fingerprint for a finding.
pub fn compute(rule: &str, path: &str, message_id: &str, line: &str) -> String {
    let anchor = normalize_anchor(line);
    let input = format!("{rule}\0{path}\0{message_id}\0{anchor}");
    format!("{:016x}", fnv1a64(input.as_bytes()))
}

fn normalize_anchor(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_stable_for_the_same_content() {
        let a = compute("E0060", "src/Foo.php", "E0060:sql", "  mysqli_query($db, $sql);  ");
        let b = compute("E0060", "src/Foo.php", "E0060:sql", "mysqli_query($db, $sql);");
        assert_eq!(a, b);
    }

    #[test]
    fn differs_by_rule_path_and_content() {
        let base = compute("E0060", "src/Foo.php", "id", "line");
        assert_ne!(base, compute("E0061", "src/Foo.php", "id", "line"));
        assert_ne!(base, compute("E0060", "src/Bar.php", "id", "line"));
        assert_ne!(base, compute("E0060", "src/Foo.php", "other", "line"));
        assert_ne!(base, compute("E0060", "src/Foo.php", "id", "other"));
    }

    #[test]
    fn does_not_depend_on_line_numbers() {
        // The function has no line parameter by design; this test documents the
        // contract so a future change does not silently reintroduce one.
        let one = compute("E0060", "src/Foo.php", "id", "$sql = $row['id'];");
        let two = compute("E0060", "src/Foo.php", "id", "$sql = $row['id'];");
        assert_eq!(one, two);
    }
}
