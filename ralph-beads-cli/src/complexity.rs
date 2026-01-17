use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Task complexity levels that determine iteration counts and validation requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Complexity {
    /// Trivial: typos, comments, whitespace (2-5 iterations, skip validation)
    Trivial,
    /// Simple: toggles, flags, removing unused code (3-10 iterations, skip validation)
    Simple,
    /// Standard: typical features (5-20 iterations, auto validation)
    Standard,
    /// Critical: auth, security, payments (8-40 iterations, required validation)
    Critical,
}

impl Default for Complexity {
    fn default() -> Self {
        Complexity::Standard
    }
}

impl fmt::Display for Complexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Complexity::Trivial => write!(f, "trivial"),
            Complexity::Simple => write!(f, "simple"),
            Complexity::Standard => write!(f, "standard"),
            Complexity::Critical => write!(f, "critical"),
        }
    }
}

impl FromStr for Complexity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trivial" => Ok(Complexity::Trivial),
            "simple" => Ok(Complexity::Simple),
            "standard" => Ok(Complexity::Standard),
            "critical" => Ok(Complexity::Critical),
            _ => Err(format!("Unknown complexity level: {}", s)),
        }
    }
}

impl Complexity {
    /// Whether validation should be enabled by default for this complexity
    pub fn default_validation(&self) -> bool {
        match self {
            Complexity::Trivial | Complexity::Simple => false,
            Complexity::Standard | Complexity::Critical => true,
        }
    }

    /// Whether validation can be skipped (critical cannot skip)
    pub fn can_skip_validation(&self) -> bool {
        !matches!(self, Complexity::Critical)
    }
}

/// Pattern matchers for complexity detection
struct ComplexityPatterns {
    trivial: Regex,
    simple: Regex,
    critical: Regex,
}

impl ComplexityPatterns {
    fn new() -> Self {
        Self {
            // TRIVIAL patterns: typo fixes, comments, whitespace, spelling, renaming
            trivial: Regex::new(
                r"(?i)(fix\s+typo|update\s+comment|rename|spelling|whitespace|typo|correct\s+spelling|documentation\s+fix|docstring)"
            ).expect("Invalid trivial regex"),

            // SIMPLE patterns: toggles, flags, removing unused, version updates
            simple: Regex::new(
                r"(?i)(add\s+(button|toggle|flag)|toggle|remove\s+unused|update\s+(version|dep)|bump\s+version|add\s+const|remove\s+dead\s+code|unused\s+import)"
            ).expect("Invalid simple regex"),

            // CRITICAL patterns: auth, security, payments, credentials, encryption
            critical: Regex::new(
                r"(?i)(auth|security|payment|migration|credential|token|encrypt|password|secret|api\s*key|oauth|jwt|session|permission|role|access\s*control|vulnerability|injection|xss|csrf|sanitiz)"
            ).expect("Invalid critical regex"),
        }
    }
}

/// Detect complexity level from task description
///
/// Uses regex pattern matching similar to the TypeScript implementation
/// but with more efficient compiled patterns.
///
/// # Examples
///
/// ```
/// use ralph_beads_cli::complexity::{detect_complexity, Complexity};
///
/// assert_eq!(detect_complexity("Fix typo in README"), Complexity::Trivial);
/// assert_eq!(detect_complexity("Add toggle button"), Complexity::Simple);
/// assert_eq!(detect_complexity("Implement user authentication"), Complexity::Critical);
/// assert_eq!(detect_complexity("Add user profile page"), Complexity::Standard);
/// ```
pub fn detect_complexity(task: &str) -> Complexity {
    let patterns = ComplexityPatterns::new();

    // Check patterns in order of specificity (critical > simple > trivial)
    // Critical patterns override others due to security importance
    if patterns.critical.is_match(task) {
        return Complexity::Critical;
    }

    // Trivial patterns for very minor changes
    if patterns.trivial.is_match(task) {
        return Complexity::Trivial;
    }

    // Simple patterns for small features
    if patterns.simple.is_match(task) {
        return Complexity::Simple;
    }

    // Default to standard for everything else
    Complexity::Standard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trivial_detection() {
        assert_eq!(detect_complexity("fix typo in README"), Complexity::Trivial);
        assert_eq!(detect_complexity("Fix typo"), Complexity::Trivial);
        assert_eq!(detect_complexity("Update comment"), Complexity::Trivial);
        assert_eq!(detect_complexity("rename variable"), Complexity::Trivial);
        assert_eq!(detect_complexity("fix spelling error"), Complexity::Trivial);
        assert_eq!(
            detect_complexity("correct whitespace issues"),
            Complexity::Trivial
        );
    }

    #[test]
    fn test_simple_detection() {
        assert_eq!(detect_complexity("add button to form"), Complexity::Simple);
        assert_eq!(
            detect_complexity("Add toggle for dark mode"),
            Complexity::Simple
        );
        assert_eq!(
            detect_complexity("remove unused imports"),
            Complexity::Simple
        );
        assert_eq!(
            detect_complexity("update version to 2.0"),
            Complexity::Simple
        );
        assert_eq!(detect_complexity("bump version"), Complexity::Simple);
    }

    #[test]
    fn test_critical_detection() {
        assert_eq!(
            detect_complexity("implement user authentication"),
            Complexity::Critical
        );
        assert_eq!(
            detect_complexity("add security headers"),
            Complexity::Critical
        );
        assert_eq!(
            detect_complexity("integrate payment gateway"),
            Complexity::Critical
        );
        assert_eq!(
            detect_complexity("database migration"),
            Complexity::Critical
        );
        assert_eq!(
            detect_complexity("store API credentials"),
            Complexity::Critical
        );
        assert_eq!(
            detect_complexity("add JWT token support"),
            Complexity::Critical
        );
        assert_eq!(detect_complexity("encrypt user data"), Complexity::Critical);
        assert_eq!(
            detect_complexity("implement password reset"),
            Complexity::Critical
        );
    }

    #[test]
    fn test_standard_detection() {
        assert_eq!(
            detect_complexity("add user profile page"),
            Complexity::Standard
        );
        assert_eq!(
            detect_complexity("implement search feature"),
            Complexity::Standard
        );
        assert_eq!(
            detect_complexity("create dashboard component"),
            Complexity::Standard
        );
        assert_eq!(
            detect_complexity("refactor data fetching"),
            Complexity::Standard
        );
    }

    #[test]
    fn test_critical_overrides_others() {
        // Even if "rename" is present, "security" should make it critical
        assert_eq!(
            detect_complexity("rename and add security check"),
            Complexity::Critical
        );

        // "authentication" should override "toggle"
        assert_eq!(
            detect_complexity("add toggle for authentication"),
            Complexity::Critical
        );
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "trivial".parse::<Complexity>().unwrap(),
            Complexity::Trivial
        );
        assert_eq!("SIMPLE".parse::<Complexity>().unwrap(), Complexity::Simple);
        assert_eq!(
            "Standard".parse::<Complexity>().unwrap(),
            Complexity::Standard
        );
        assert_eq!(
            "CRITICAL".parse::<Complexity>().unwrap(),
            Complexity::Critical
        );
        assert!("invalid".parse::<Complexity>().is_err());
    }

    #[test]
    fn test_display() {
        assert_eq!(Complexity::Trivial.to_string(), "trivial");
        assert_eq!(Complexity::Simple.to_string(), "simple");
        assert_eq!(Complexity::Standard.to_string(), "standard");
        assert_eq!(Complexity::Critical.to_string(), "critical");
    }

    #[test]
    fn test_validation_defaults() {
        assert!(!Complexity::Trivial.default_validation());
        assert!(!Complexity::Simple.default_validation());
        assert!(Complexity::Standard.default_validation());
        assert!(Complexity::Critical.default_validation());

        assert!(Complexity::Trivial.can_skip_validation());
        assert!(Complexity::Simple.can_skip_validation());
        assert!(Complexity::Standard.can_skip_validation());
        assert!(!Complexity::Critical.can_skip_validation());
    }
}
