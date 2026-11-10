//! Static metadata attached to every finding.
//!
//! Metadata lives outside the individual rule implementations so that adding a
//! rule does not require editing every output format, and so that text, JSON,
//! SARIF and Code Climate all agree on severity, category and identity.

use serde::{Deserialize, Serialize};

/// How much damage a finding can cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Minor,
    Major,
    Critical,
    Blocker,
}

impl Default for Severity {
    fn default() -> Self {
        Self::Info
    }
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Critical => "critical",
            Self::Blocker => "blocker",
        }
    }

    /// SARIF only understands `note`, `warning` and `error`.
    pub fn sarif_level(self) -> &'static str {
        match self {
            Self::Info | Self::Minor => "note",
            Self::Major => "warning",
            Self::Critical | Self::Blocker => "error",
        }
    }

    /// Code Climate severity names match ours exactly.
    pub fn code_climate(self) -> &'static str {
        self.as_str()
    }

    /// Relative weight used by later risk scoring. Kept here so every consumer
    /// agrees on ordering.
    #[allow(dead_code)]
    pub fn weight(self) -> u8 {
        match self {
            Self::Info => 1,
            Self::Minor => 2,
            Self::Major => 3,
            Self::Critical => 4,
            Self::Blocker => 5,
        }
    }
}

/// The part of the product a finding belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    CodeHealth,
    Verification,
    Resource,
    Lifecycle,
    Security,
}

impl Default for Category {
    fn default() -> Self {
        Self::CodeHealth
    }
}

impl Category {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CodeHealth => "code-health",
            Self::Verification => "verification",
            Self::Resource => "resource",
            Self::Lifecycle => "lifecycle",
            Self::Security => "security",
        }
    }

    /// Code Climate's fixed category vocabulary.
    pub fn code_climate(self) -> &'static [&'static str] {
        match self {
            Self::CodeHealth => &["Clarity"],
            Self::Verification => &["Bug Risk"],
            Self::Resource => &["Performance"],
            Self::Lifecycle => &["Bug Risk"],
            Self::Security => &["Security"],
        }
    }
}

/// How much the analyser trusts a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    #[default]
    Low,
    Medium,
    High,
}

impl Confidence {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Rough fix size, used to separate quick wins from projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    #[default]
    Trivial,
    Small,
    Medium,
    Large,
}

impl Effort {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trivial => "trivial",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

/// The static metadata for a rule code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleMeta {
    pub severity: Severity,
    pub category: Category,
    pub confidence: Confidence,
    pub effort: Effort,
    pub cwe: Option<&'static str>,
    pub owasp: Option<&'static str>,
}

/// Metadata as carried by an individual finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingMeta {
    pub severity: Severity,
    pub category: Category,
    pub confidence: Confidence,
    pub effort: Effort,
    /// Stable identity independent of line numbers.
    pub fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owasp: Option<String>,
    pub help_url: String,
}

impl Default for FindingMeta {
    fn default() -> Self {
        Self {
            severity: Severity::Info,
            category: Category::CodeHealth,
            confidence: Confidence::Low,
            effort: Effort::Trivial,
            fingerprint: String::new(),
            cwe: None,
            owasp: None,
            help_url: String::new(),
        }
    }
}

impl FindingMeta {
    /// Build finding metadata for a rule code and a computed fingerprint.
    pub fn from_rule(code: &str, fingerprint: String) -> Self {
        let meta = for_code(code);
        Self {
            severity: meta.severity,
            category: meta.category,
            confidence: meta.confidence,
            effort: meta.effort,
            fingerprint,
            cwe: meta.cwe.map(String::from),
            owasp: meta.owasp.map(String::from),
            help_url: help_url(code),
        }
    }

    /// A proven request-to-sink flow is high confidence even when the rule's
    /// fallback heuristic is not.
    pub fn with_proven_taint(mut self) -> Self {
        self.confidence = Confidence::High;
        self
    }

    /// A parameter-derived taint flow is a lead, not proof: the sink is
    /// reachable from a function argument, but nothing here shows that a
    /// caller supplies untrusted data. Keep it visible but deprioritise it
    /// (low confidence, minor severity) so leads do not read as confirmed
    /// vulnerabilities.
    pub fn with_taint_lead(mut self) -> Self {
        self.confidence = Confidence::Low;
        self.severity = Severity::Minor;
        self
    }
}

/// Metadata for a rule code. Every registered code resolves to a value; unknown
/// or malformed codes fall back to the security defaults rather than panicking.
pub fn for_code(code: &str) -> RuleMeta {
    let number = code_number(code);
    let category = category_for(number);
    let (mut severity, confidence, effort) = defaults_for(category);

    if category == Category::Security {
        severity = security_severity(code);
    }

    let (cwe, owasp) = standards(code);

    RuleMeta {
        severity,
        category,
        confidence,
        effort,
        cwe,
        owasp,
    }
}

/// Documentation URL for a rule code.
pub fn help_url(code: &str) -> String {
    let number = code.trim_start_matches('E').trim_start_matches('0');
    let number = if number.is_empty() { "0" } else { number };
    format!(
        "https://github.com/denzyldick/phanalist/blob/main/src/rules/examples/e{number}/e{number}.md"
    )
}

fn code_number(code: &str) -> u32 {
    code.trim_start_matches('E').parse().unwrap_or(0)
}

fn category_for(number: u32) -> Category {
    match number {
        0..=30 => Category::CodeHealth,
        31..=43 => Category::Verification,
        44..=46 => Category::Resource,
        47..=52 => Category::Lifecycle,
        _ => Category::Security,
    }
}

fn defaults_for(category: Category) -> (Severity, Confidence, Effort) {
    match category {
        Category::CodeHealth => (Severity::Minor, Confidence::High, Effort::Small),
        Category::Verification => (Severity::Major, Confidence::High, Effort::Medium),
        Category::Resource => (Severity::Major, Confidence::Medium, Effort::Medium),
        Category::Lifecycle => (Severity::Major, Confidence::High, Effort::Small),
        Category::Security => (Severity::Critical, Confidence::Medium, Effort::Small),
    }
}

/// Security findings are critical by default; these are the deliberate
/// exceptions where the impact or the signal is weaker.
fn security_severity(code: &str) -> Severity {
    if matches!(
        code,
        "E0058"
            | "E0076"
            | "E0077"
            | "E0081"
            | "E0082"
            | "E0085"
            | "E0086"
            | "E0096"
            | "E0097"
            | "E0109"
            | "E0114"
            | "E0119"
            | "E0135"
    ) {
        return Severity::Minor;
    }

    if matches!(
        code,
        "E0054"
            | "E0056"
            | "E0064"
            | "E0066"
            | "E0067"
            | "E0073"
            | "E0074"
            | "E0075"
            | "E0078"
            | "E0079"
            | "E0083"
            | "E0088"
            | "E0089"
            | "E0091"
            | "E0094"
            | "E0095"
            | "E0098"
            | "E0099"
            | "E0100"
            | "E0104"
            | "E0107"
            | "E0108"
            | "E0111"
            | "E0113"
            | "E0118"
            | "E0125"
            | "E0132"
            | "E0133"
            | "E0134"
    ) {
        return Severity::Major;
    }

    Severity::Critical
}

fn standards(code: &str) -> (Option<&'static str>, Option<&'static str>) {
    match code {
        "E0053" | "E0069" | "E0101" | "E0106" => (Some("CWE-502"), Some("A08:2021")),
        "E0054" | "E0059" | "E0068" | "E0078" | "E0088" | "E0120" | "E0121" => {
            (Some("CWE-95"), Some("A03:2021"))
        }
        "E0055" | "E0070" | "E0105" | "E0122" => (Some("CWE-98"), Some("A03:2021")),
        "E0056" | "E0074" | "E0107" => (Some("CWE-338"), Some("A02:2021")),
        "E0057" | "E0073" | "E0080" | "E0124" => (Some("CWE-916"), Some("A02:2021")),
        "E0058" | "E0096" | "E0097" | "E0119" => (Some("CWE-614"), Some("A05:2021")),
        "E0060" | "E0084" | "E0110" | "E0117" => (Some("CWE-89"), Some("A03:2021")),
        "E0061" | "E0123" => (Some("CWE-79"), Some("A03:2021")),
        "E0062" | "E0087" | "E0102" | "E0116" | "E0127" | "E0128" | "E0129" | "E0130"
        | "E0131" => (Some("CWE-78"), Some("A03:2021")),
        "E0063" => (Some("CWE-90"), Some("A03:2021")),
        "E0064" => (Some("CWE-643"), Some("A03:2021")),
        "E0065" => (Some("CWE-943"), Some("A03:2021")),
        "E0066" | "E0104" | "E0132" => (Some("CWE-113"), Some("A03:2021")),
        "E0067" | "E0134" => (Some("CWE-93"), Some("A03:2021")),
        "E0071" | "E0112" => (Some("CWE-434"), Some("A04:2021")),
        "E0072" | "E0090" | "E0109" | "E0126" => (Some("CWE-22"), Some("A01:2021")),
        "E0075" | "E0113" => (Some("CWE-384"), Some("A07:2021")),
        "E0076" | "E0091" | "E0114" | "E0135" => (Some("CWE-200"), Some("A05:2021")),
        "E0077" => (Some("CWE-209"), Some("A05:2021")),
        "E0079" | "E0100" | "E0108" => (Some("CWE-470"), Some("A03:2021")),
        "E0083" | "E0125" => (Some("CWE-601"), Some("A01:2021")),
        "E0089" | "E0118" => (Some("CWE-117"), Some("A09:2021")),
        "E0092" | "E0115" => (Some("CWE-611"), Some("A05:2021")),
        "E0093" => (Some("CWE-1336"), Some("A03:2021")),
        "E0094" => (Some("CWE-1333"), Some("A05:2021")),
        "E0095" => (Some("CWE-208"), Some("A02:2021")),
        "E0098" => (Some("CWE-598"), Some("A07:2021")),
        "E0103" => (Some("CWE-918"), Some("A10:2021")),
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::all_rules;

    #[test]
    fn every_registered_rule_has_metadata() {
        for code in all_rules().keys() {
            let meta = for_code(code);
            assert_eq!(
                meta.category,
                category_for(code_number(code)),
                "category drift for {code}"
            );
            assert!(!help_url(code).is_empty(), "missing help url for {code}");
        }
    }

    #[test]
    fn security_rules_are_never_informational() {
        for code in all_rules().keys() {
            let meta = for_code(code);
            if meta.category == Category::Security {
                assert!(
                    meta.severity >= Severity::Minor,
                    "{code} is {} but security findings must be at least minor",
                    meta.severity.as_str()
                );
            }
        }
    }

    #[test]
    fn categories_follow_rule_ranges() {
        assert_eq!(for_code("E0001").category, Category::CodeHealth);
        assert_eq!(for_code("E0031").category, Category::Verification);
        assert_eq!(for_code("E0044").category, Category::Resource);
        assert_eq!(for_code("E0047").category, Category::Lifecycle);
        assert_eq!(for_code("E0060").category, Category::Security);
    }

    #[test]
    fn injection_rules_carry_standards() {
        let meta = for_code("E0060");
        assert_eq!(meta.cwe, Some("CWE-89"));
        assert_eq!(meta.owasp, Some("A03:2021"));
    }

    #[test]
    fn finding_meta_from_rule_is_complete() {
        let meta = FindingMeta::from_rule("E0060", String::from("abc"));
        assert_eq!(meta.severity, Severity::Critical);
        assert_eq!(meta.category, Category::Security);
        assert_eq!(meta.fingerprint, "abc");
        assert!(meta.help_url.contains("e60"));
    }
}
