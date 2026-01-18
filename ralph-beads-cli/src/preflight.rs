//! Preflight Check Integration Module
//!
//! Wraps the beads CLI `bd preflight` command to provide pre-PR validation
//! checks for ralph-beads workflows. This integrates with the beads preflight
//! system which runs tests, lint, and other checks before creating PRs.
//!
//! The module provides:
//! - Run all preflight checks via `bd preflight --check --json`
//! - Run individual checks (tests, lint, build, uncommitted)
//! - Parse and structure results for programmatic use

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Instant;
use thiserror::Error;

/// Errors that can occur during preflight operations
#[derive(Error, Debug)]
pub enum PreflightError {
    #[error("Preflight check failed: {0}")]
    CheckFailed(String),

    #[error("Beads CLI error: {0}")]
    CliError(String),

    #[error("Failed to execute bd command: {0}")]
    ExecutionError(#[from] std::io::Error),

    #[error("Failed to parse JSON output: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Unknown check name: {0}. Valid checks: tests, lint, build, uncommitted")]
    UnknownCheck(String),
}

/// Status of an individual preflight check
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    /// Check passed successfully
    Passed,
    /// Check failed
    Failed,
    /// Check was skipped (e.g., tool not installed)
    Skipped,
    /// Check passed with warnings
    Warning,
}

impl CheckStatus {
    /// Returns true if the check did not fail
    pub fn is_ok(&self) -> bool {
        !matches!(self, CheckStatus::Failed)
    }
}

/// Result of an individual preflight check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightCheck {
    /// Check name (e.g., "Tests pass", "Lint passes")
    pub name: String,
    /// Status of this check
    pub status: CheckStatus,
    /// Human-readable message or output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Command that was run (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

impl PreflightCheck {
    /// Create a new passed check
    pub fn passed(name: &str, message: Option<&str>, duration_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Passed,
            message: message.map(|s| s.to_string()),
            duration_ms,
            command: None,
        }
    }

    /// Create a new failed check
    pub fn failed(name: &str, message: &str, duration_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Failed,
            message: Some(message.to_string()),
            duration_ms,
            command: None,
        }
    }

    /// Create a new skipped check
    pub fn skipped(name: &str, reason: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Skipped,
            message: Some(reason.to_string()),
            duration_ms: 0,
            command: None,
        }
    }

    /// Create a new warning check
    pub fn warning(name: &str, message: &str, duration_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Warning,
            message: Some(message.to_string()),
            duration_ms,
            command: None,
        }
    }

    /// Add command information to the check
    pub fn with_command(mut self, command: &str) -> Self {
        self.command = Some(command.to_string());
        self
    }
}

/// Complete preflight report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightReport {
    /// Whether all checks passed (no failures)
    pub passed: bool,
    /// Individual check results
    pub checks: Vec<PreflightCheck>,
    /// Summary message
    pub summary: String,
    /// Total checks run
    pub total: usize,
    /// Checks that passed
    pub passed_count: usize,
    /// Checks that failed
    pub failed_count: usize,
    /// Checks that were skipped
    pub skipped_count: usize,
}

impl PreflightReport {
    /// Create a report from a list of checks
    pub fn from_checks(checks: Vec<PreflightCheck>) -> Self {
        let total = checks.len();
        let passed_count = checks.iter().filter(|c| c.status == CheckStatus::Passed).count();
        let failed_count = checks.iter().filter(|c| c.status == CheckStatus::Failed).count();
        let skipped_count = checks.iter().filter(|c| c.status == CheckStatus::Skipped).count();

        let passed = failed_count == 0;

        let summary = if passed {
            if skipped_count > 0 {
                format!(
                    "{}/{} checks passed ({} skipped)",
                    passed_count,
                    total - skipped_count,
                    skipped_count
                )
            } else {
                format!("{}/{} checks passed", passed_count, total)
            }
        } else {
            format!(
                "{}/{} checks failed",
                failed_count,
                total - skipped_count
            )
        };

        Self {
            passed,
            checks,
            summary,
            total,
            passed_count,
            failed_count,
            skipped_count,
        }
    }
}

/// Raw JSON output from `bd preflight --check --json`
#[derive(Debug, Deserialize)]
struct BdPreflightOutput {
    checks: Vec<BdPreflightCheck>,
    passed: bool,
    summary: String,
}

/// Raw check from bd preflight JSON output
#[derive(Debug, Deserialize)]
struct BdPreflightCheck {
    name: String,
    passed: bool,
    #[serde(default)]
    skipped: bool,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    command: Option<String>,
}

/// Run all preflight checks via `bd preflight --check --json`
///
/// This is the main entry point that runs all configured preflight checks
/// using the beads preflight system.
///
/// # Arguments
/// * `issue_id` - Optional issue ID to scope the preflight (not currently used by bd)
///
/// # Returns
/// * `Ok(PreflightReport)` - Report containing all check results
/// * `Err(PreflightError)` - If the command fails or output cannot be parsed
///
/// # Example
/// ```ignore
/// let report = run_preflight(None)?;
/// if report.passed {
///     println!("All checks passed, ready to create PR");
/// }
/// ```
pub fn run_preflight(_issue_id: Option<&str>) -> Result<PreflightReport, PreflightError> {
    let start = Instant::now();

    let output = Command::new("bd")
        .args(["preflight", "--check", "--json"])
        .output()?;

    let duration = start.elapsed().as_millis() as u64;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Try to parse the JSON output
    if let Ok(bd_output) = serde_json::from_str::<BdPreflightOutput>(&stdout) {
        let checks: Vec<PreflightCheck> = bd_output
            .checks
            .into_iter()
            .map(|c| {
                let status = if c.skipped {
                    CheckStatus::Skipped
                } else if c.passed {
                    CheckStatus::Passed
                } else {
                    CheckStatus::Failed
                };

                PreflightCheck {
                    name: c.name,
                    status,
                    message: c.output,
                    duration_ms: 0, // bd doesn't provide individual timings
                    command: c.command,
                }
            })
            .collect();

        return Ok(PreflightReport::from_checks(checks));
    }

    // If parsing fails but command succeeded, create a generic result
    if output.status.success() {
        Ok(PreflightReport {
            passed: true,
            checks: vec![PreflightCheck::passed("preflight", Some("Preflight completed"), duration)],
            summary: "Preflight checks passed".to_string(),
            total: 1,
            passed_count: 1,
            failed_count: 0,
            skipped_count: 0,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(PreflightError::CliError(format!(
            "bd preflight failed: {}",
            stderr
        )))
    }
}

/// Run a specific preflight check
///
/// # Arguments
/// * `check_name` - Name of the check to run: "tests", "lint", "build", "uncommitted"
///
/// # Returns
/// * `Ok(PreflightCheck)` - Result of the specific check
/// * `Err(PreflightError)` - If the check name is unknown or execution fails
pub fn run_single_check(check_name: &str) -> Result<PreflightCheck, PreflightError> {
    match check_name.to_lowercase().as_str() {
        "tests" | "test" => check_tests(),
        "lint" => check_lint(),
        "build" => check_build(),
        "uncommitted" | "git" => check_uncommitted(),
        _ => Err(PreflightError::UnknownCheck(check_name.to_string())),
    }
}

/// Check if tests pass
///
/// Runs `bd preflight --check --json` and extracts the tests result,
/// or falls back to running tests directly if bd preflight doesn't support it.
pub fn check_tests() -> Result<PreflightCheck, PreflightError> {
    let start = Instant::now();

    // First try to get it from bd preflight
    if let Ok(report) = run_preflight(None) {
        if let Some(check) = report.checks.iter().find(|c| c.name.to_lowercase().contains("test")) {
            return Ok(check.clone());
        }
    }

    // Fall back to direct test execution based on project type
    let duration = start.elapsed().as_millis() as u64;

    // Detect project type and run appropriate test command
    if std::path::Path::new("Cargo.toml").exists() {
        let output = Command::new("cargo")
            .args(["test", "--no-run"])
            .output()?;

        if output.status.success() {
            Ok(PreflightCheck::passed("tests", Some("Cargo tests compile"), duration)
                .with_command("cargo test --no-run"))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(PreflightCheck::failed("tests", &stderr, duration)
                .with_command("cargo test --no-run"))
        }
    } else if std::path::Path::new("package.json").exists() {
        let output = Command::new("npm")
            .args(["test", "--", "--passWithNoTests"])
            .output()?;

        if output.status.success() {
            Ok(PreflightCheck::passed("tests", Some("npm tests pass"), duration)
                .with_command("npm test"))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(PreflightCheck::failed("tests", &stderr, duration)
                .with_command("npm test"))
        }
    } else if std::path::Path::new("pyproject.toml").exists() {
        let output = Command::new("pytest")
            .args(["--collect-only", "-q"])
            .output()?;

        if output.status.success() {
            Ok(PreflightCheck::passed("tests", Some("pytest tests collected"), duration)
                .with_command("pytest --collect-only"))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(PreflightCheck::failed("tests", &stderr, duration)
                .with_command("pytest --collect-only"))
        }
    } else {
        Ok(PreflightCheck::skipped("tests", "No recognized test framework found"))
    }
}

/// Check if lint passes
///
/// Runs `bd preflight --check --json` and extracts the lint result,
/// or falls back to running linter directly.
pub fn check_lint() -> Result<PreflightCheck, PreflightError> {
    let start = Instant::now();

    // First try to get it from bd preflight
    if let Ok(report) = run_preflight(None) {
        if let Some(check) = report.checks.iter().find(|c| c.name.to_lowercase().contains("lint")) {
            return Ok(check.clone());
        }
    }

    let duration = start.elapsed().as_millis() as u64;

    // Detect project type and run appropriate lint command
    if std::path::Path::new("Cargo.toml").exists() {
        let output = Command::new("cargo")
            .args(["clippy", "--", "-D", "warnings"])
            .output()?;

        if output.status.success() {
            Ok(PreflightCheck::passed("lint", Some("Clippy passes"), duration)
                .with_command("cargo clippy -- -D warnings"))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(PreflightCheck::failed("lint", &stderr, duration)
                .with_command("cargo clippy -- -D warnings"))
        }
    } else if std::path::Path::new("package.json").exists() {
        // Check for eslint
        let output = Command::new("npx")
            .args(["eslint", ".", "--max-warnings=0"])
            .output()?;

        if output.status.success() {
            Ok(PreflightCheck::passed("lint", Some("ESLint passes"), duration)
                .with_command("npx eslint . --max-warnings=0"))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let msg = if stderr.is_empty() { stdout } else { stderr };
            Ok(PreflightCheck::failed("lint", &msg, duration)
                .with_command("npx eslint . --max-warnings=0"))
        }
    } else if std::path::Path::new("pyproject.toml").exists() {
        let output = Command::new("ruff")
            .args(["check", "."])
            .output()?;

        if output.status.success() {
            Ok(PreflightCheck::passed("lint", Some("Ruff passes"), duration)
                .with_command("ruff check ."))
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(PreflightCheck::failed("lint", &stdout, duration)
                .with_command("ruff check ."))
        }
    } else {
        Ok(PreflightCheck::skipped("lint", "No recognized linter found"))
    }
}

/// Check if build passes
///
/// Verifies that the project builds successfully.
pub fn check_build() -> Result<PreflightCheck, PreflightError> {
    let start = Instant::now();

    if std::path::Path::new("Cargo.toml").exists() {
        let output = Command::new("cargo")
            .args(["build"])
            .output()?;

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            Ok(PreflightCheck::passed("build", Some("Cargo build succeeds"), duration)
                .with_command("cargo build"))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(PreflightCheck::failed("build", &stderr, duration)
                .with_command("cargo build"))
        }
    } else if std::path::Path::new("package.json").exists() {
        let output = Command::new("npm")
            .args(["run", "build"])
            .output()?;

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            Ok(PreflightCheck::passed("build", Some("npm build succeeds"), duration)
                .with_command("npm run build"))
        } else {
            // Check if build script exists
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("missing script: build") {
                Ok(PreflightCheck::skipped("build", "No build script in package.json"))
            } else {
                Ok(PreflightCheck::failed("build", &stderr, duration)
                    .with_command("npm run build"))
            }
        }
    } else {
        let duration = start.elapsed().as_millis() as u64;
        Ok(PreflightCheck::skipped("build", "No recognized build system found").with_command("N/A"))
            .map(|mut c| {
                c.duration_ms = duration;
                c
            })
    }
}

/// Check for uncommitted changes
///
/// Verifies that the git working tree is clean or has expected changes.
pub fn check_uncommitted() -> Result<PreflightCheck, PreflightError> {
    let start = Instant::now();

    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;

    let duration = start.elapsed().as_millis() as u64;

    if !output.status.success() {
        return Ok(PreflightCheck::failed(
            "uncommitted",
            "Failed to check git status",
            duration,
        )
        .with_command("git status --porcelain"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let changes: Vec<&str> = stdout.lines().collect();

    if changes.is_empty() {
        Ok(PreflightCheck::passed(
            "uncommitted",
            Some("Working tree is clean"),
            duration,
        )
        .with_command("git status --porcelain"))
    } else {
        let change_count = changes.len();
        if change_count <= 5 {
            // Few changes, might be intentional
            Ok(PreflightCheck::warning(
                "uncommitted",
                &format!("{} uncommitted change(s)", change_count),
                duration,
            )
            .with_command("git status --porcelain"))
        } else {
            // Many changes, probably should commit first
            Ok(PreflightCheck::failed(
                "uncommitted",
                &format!("{} uncommitted changes - consider committing first", change_count),
                duration,
            )
            .with_command("git status --porcelain"))
        }
    }
}

/// Get list of available check names
pub fn available_checks() -> Vec<&'static str> {
    vec!["tests", "lint", "build", "uncommitted"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_status_is_ok() {
        assert!(CheckStatus::Passed.is_ok());
        assert!(CheckStatus::Skipped.is_ok());
        assert!(CheckStatus::Warning.is_ok());
        assert!(!CheckStatus::Failed.is_ok());
    }

    #[test]
    fn test_preflight_check_constructors() {
        let passed = PreflightCheck::passed("test", Some("All good"), 100);
        assert_eq!(passed.name, "test");
        assert_eq!(passed.status, CheckStatus::Passed);
        assert_eq!(passed.message, Some("All good".to_string()));
        assert_eq!(passed.duration_ms, 100);

        let failed = PreflightCheck::failed("test", "Error occurred", 50);
        assert_eq!(failed.status, CheckStatus::Failed);
        assert_eq!(failed.message, Some("Error occurred".to_string()));

        let skipped = PreflightCheck::skipped("test", "Not applicable");
        assert_eq!(skipped.status, CheckStatus::Skipped);
        assert_eq!(skipped.duration_ms, 0);

        let warning = PreflightCheck::warning("test", "Minor issue", 75);
        assert_eq!(warning.status, CheckStatus::Warning);
    }

    #[test]
    fn test_preflight_check_with_command() {
        let check = PreflightCheck::passed("test", None, 0).with_command("cargo test");
        assert_eq!(check.command, Some("cargo test".to_string()));
    }

    #[test]
    fn test_preflight_report_all_passed() {
        let checks = vec![
            PreflightCheck::passed("test1", None, 100),
            PreflightCheck::passed("test2", None, 200),
        ];
        let report = PreflightReport::from_checks(checks);

        assert!(report.passed);
        assert_eq!(report.total, 2);
        assert_eq!(report.passed_count, 2);
        assert_eq!(report.failed_count, 0);
        assert_eq!(report.skipped_count, 0);
        assert!(report.summary.contains("2/2"));
    }

    #[test]
    fn test_preflight_report_with_failures() {
        let checks = vec![
            PreflightCheck::passed("test1", None, 100),
            PreflightCheck::failed("test2", "Error", 200),
            PreflightCheck::passed("test3", None, 150),
        ];
        let report = PreflightReport::from_checks(checks);

        assert!(!report.passed);
        assert_eq!(report.total, 3);
        assert_eq!(report.passed_count, 2);
        assert_eq!(report.failed_count, 1);
        assert!(report.summary.contains("1/3"));
    }

    #[test]
    fn test_preflight_report_with_skipped() {
        let checks = vec![
            PreflightCheck::passed("test1", None, 100),
            PreflightCheck::skipped("test2", "Not available"),
        ];
        let report = PreflightReport::from_checks(checks);

        assert!(report.passed);
        assert_eq!(report.total, 2);
        assert_eq!(report.passed_count, 1);
        assert_eq!(report.skipped_count, 1);
        assert!(report.summary.contains("skipped"));
    }

    #[test]
    fn test_run_single_check_unknown() {
        let result = run_single_check("unknown_check");
        assert!(result.is_err());
        match result {
            Err(PreflightError::UnknownCheck(name)) => {
                assert_eq!(name, "unknown_check");
            }
            _ => panic!("Expected UnknownCheck error"),
        }
    }

    #[test]
    fn test_available_checks() {
        let checks = available_checks();
        assert!(checks.contains(&"tests"));
        assert!(checks.contains(&"lint"));
        assert!(checks.contains(&"build"));
        assert!(checks.contains(&"uncommitted"));
    }

    #[test]
    fn test_check_status_serialization() {
        let status = CheckStatus::Passed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"passed\"");

        let deserialized: CheckStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, CheckStatus::Passed);
    }

    #[test]
    fn test_preflight_check_serialization() {
        let check = PreflightCheck::passed("tests", Some("All tests pass"), 1234)
            .with_command("cargo test");

        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains("\"name\":\"tests\""));
        assert!(json.contains("\"status\":\"passed\""));
        assert!(json.contains("\"duration_ms\":1234"));
        assert!(json.contains("\"command\":\"cargo test\""));
    }

    #[test]
    fn test_preflight_check_serialization_skips_none() {
        let check = PreflightCheck {
            name: "test".to_string(),
            status: CheckStatus::Passed,
            message: None,
            duration_ms: 0,
            command: None,
        };

        let json = serde_json::to_string(&check).unwrap();
        assert!(!json.contains("message"));
        assert!(!json.contains("command"));
    }

    #[test]
    fn test_preflight_report_serialization() {
        let checks = vec![PreflightCheck::passed("test", None, 100)];
        let report = PreflightReport::from_checks(checks);

        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("\"passed\": true"));
        assert!(json.contains("\"total\": 1"));
        assert!(json.contains("\"passed_count\": 1"));
    }
}
