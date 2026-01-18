//! Lint Integration Module
//!
//! Provides issue quality checking by wrapping the beads CLI `bd lint` command.
//! Checks issues for missing recommended sections based on issue type.
//!
//! The module provides:
//! - Lint single issues via `bd lint <id>`
//! - Lint epics and their children
//! - Lint all open issues
//! - Structured lint results with severity levels and suggestions

use serde::{Deserialize, Serialize};
use std::process::Command;
use thiserror::Error;

/// Errors that can occur during lint operations
#[derive(Error, Debug)]
pub enum LintError {
    #[error("Lint check failed: {0}")]
    CheckFailed(String),

    #[error("Beads CLI error: {0}")]
    CliError(String),

    #[error("Failed to execute bd command: {0}")]
    ExecutionError(#[from] std::io::Error),

    #[error("Failed to parse JSON output: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Issue not found: {0}")]
    IssueNotFound(String),
}

/// Severity level for lint results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    /// Critical issue that should block workflow
    Error,
    /// Issue that should be addressed
    Warning,
    /// Informational suggestion
    Info,
}

impl std::fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintSeverity::Error => write!(f, "error"),
            LintSeverity::Warning => write!(f, "warning"),
            LintSeverity::Info => write!(f, "info"),
        }
    }
}

/// Types of lint rules that can be checked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintRule {
    /// Issue is missing a description/body
    MissingDescription,
    /// Issue is missing acceptance criteria section
    MissingAcceptanceCriteria,
    /// Issue has no priority set
    MissingPriority,
    /// Task has no parent epic
    OrphanedTask,
    /// Circular dependency detected
    CircularDependency,
    /// Issue has had no updates in a long time
    StaleIssue,
    /// Bug is missing steps to reproduce
    MissingStepsToReproduce,
    /// Epic is missing success criteria
    MissingSuccessCriteria,
    /// Generic missing section
    MissingSection,
}

impl std::fmt::Display for LintRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintRule::MissingDescription => write!(f, "missing-description"),
            LintRule::MissingAcceptanceCriteria => write!(f, "missing-acceptance-criteria"),
            LintRule::MissingPriority => write!(f, "missing-priority"),
            LintRule::OrphanedTask => write!(f, "orphaned-task"),
            LintRule::CircularDependency => write!(f, "circular-dependency"),
            LintRule::StaleIssue => write!(f, "stale-issue"),
            LintRule::MissingStepsToReproduce => write!(f, "missing-steps-to-reproduce"),
            LintRule::MissingSuccessCriteria => write!(f, "missing-success-criteria"),
            LintRule::MissingSection => write!(f, "missing-section"),
        }
    }
}

/// Individual lint result for an issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintResult {
    /// Issue ID that the lint applies to
    pub issue_id: String,
    /// Type of lint rule violated
    pub rule: LintRule,
    /// Severity of the violation
    pub severity: LintSeverity,
    /// Human-readable message describing the issue
    pub message: String,
    /// Suggested fix for the issue
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl LintResult {
    /// Create a new lint result
    pub fn new(issue_id: &str, rule: LintRule, severity: LintSeverity, message: &str) -> Self {
        Self {
            issue_id: issue_id.to_string(),
            rule,
            severity,
            message: message.to_string(),
            suggestion: None,
        }
    }

    /// Add a suggestion for fixing the issue
    pub fn with_suggestion(mut self, suggestion: &str) -> Self {
        self.suggestion = Some(suggestion.to_string());
        self
    }
}

/// Complete lint report for one or more issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintReport {
    /// All lint results found
    pub results: Vec<LintResult>,
    /// Number of errors found
    pub errors: usize,
    /// Number of warnings found
    pub warnings: usize,
    /// Number of info items found
    pub infos: usize,
    /// Whether all checks passed (no errors or warnings)
    pub passed: bool,
    /// Summary message
    pub summary: String,
}

impl LintReport {
    /// Create a report from a list of lint results
    pub fn from_results(results: Vec<LintResult>) -> Self {
        let errors = results
            .iter()
            .filter(|r| r.severity == LintSeverity::Error)
            .count();
        let warnings = results
            .iter()
            .filter(|r| r.severity == LintSeverity::Warning)
            .count();
        let infos = results
            .iter()
            .filter(|r| r.severity == LintSeverity::Info)
            .count();

        let passed = errors == 0 && warnings == 0;

        let summary = if passed {
            if infos > 0 {
                format!("Passed with {} info item(s)", infos)
            } else {
                "All lint checks passed".to_string()
            }
        } else {
            format!("{} error(s), {} warning(s) found", errors, warnings)
        };

        Self {
            results,
            errors,
            warnings,
            infos,
            passed,
            summary,
        }
    }

    /// Create an empty passing report
    pub fn empty() -> Self {
        Self {
            results: Vec::new(),
            errors: 0,
            warnings: 0,
            infos: 0,
            passed: true,
            summary: "No issues found".to_string(),
        }
    }

    /// Merge another report into this one
    pub fn merge(&mut self, other: LintReport) {
        self.results.extend(other.results);
        self.errors += other.errors;
        self.warnings += other.warnings;
        self.infos += other.infos;
        self.passed = self.errors == 0 && self.warnings == 0;
        self.summary = if self.passed {
            if self.infos > 0 {
                format!("Passed with {} info item(s)", self.infos)
            } else {
                "All lint checks passed".to_string()
            }
        } else {
            format!(
                "{} error(s), {} warning(s) found",
                self.errors, self.warnings
            )
        };
    }
}

/// Raw JSON output from `bd lint --json`
#[derive(Debug, Deserialize)]
struct BdLintOutput {
    #[serde(default)]
    issues: Vec<BdLintIssue>,
    #[serde(default)]
    passed: bool,
}

/// Raw lint issue from bd lint JSON output
#[derive(Debug, Deserialize)]
struct BdLintIssue {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    missing_sections: Vec<String>,
    #[serde(default, rename = "type")]
    issue_type: Option<String>,
}

/// Lint a single issue by ID
///
/// # Arguments
/// * `issue_id` - The ID of the issue to lint
///
/// # Returns
/// * `Ok(Vec<LintResult>)` - List of lint violations found
/// * `Err(LintError)` - If the issue cannot be found or linted
pub fn lint_issue(issue_id: &str) -> Result<Vec<LintResult>, LintError> {
    let output = Command::new("bd")
        .args(["lint", issue_id, "--json"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check for "not found" errors
    if stderr.contains("not found") || stderr.contains("No issue found") {
        return Err(LintError::IssueNotFound(issue_id.to_string()));
    }

    // Try to parse the JSON output
    if let Ok(bd_output) = serde_json::from_str::<BdLintOutput>(&stdout) {
        let results = convert_bd_lint_output(bd_output);
        return Ok(results);
    }

    // If bd lint didn't return JSON but succeeded, assume no issues
    if output.status.success() {
        // Check if there's text output indicating issues
        if stdout.contains("missing") {
            // Parse text output format
            return parse_text_lint_output(&stdout, issue_id);
        }
        return Ok(Vec::new());
    }

    Err(LintError::CliError(format!("bd lint failed: {}", stderr)))
}

/// Lint an epic and all its children
///
/// # Arguments
/// * `epic_id` - The ID of the epic to lint
///
/// # Returns
/// * `Ok(LintReport)` - Complete lint report for the epic and children
/// * `Err(LintError)` - If linting fails
pub fn lint_epic(epic_id: &str) -> Result<LintReport, LintError> {
    // First lint the epic itself
    let epic_results = lint_issue(epic_id)?;
    let mut report = LintReport::from_results(epic_results);

    // Get children of the epic
    let output = Command::new("bd")
        .args(["list", "--parent", epic_id, "--json"])
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Ok(children) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
            for child in children {
                if let Some(child_id) = child.get("id").and_then(|v| v.as_str()) {
                    if let Ok(child_results) = lint_issue(child_id) {
                        let child_report = LintReport::from_results(child_results);
                        report.merge(child_report);
                    }
                }
            }
        }
    }

    Ok(report)
}

/// Lint all open issues in the project
///
/// # Returns
/// * `Ok(LintReport)` - Complete lint report for all issues
/// * `Err(LintError)` - If linting fails
pub fn lint_all() -> Result<LintReport, LintError> {
    let output = Command::new("bd").args(["lint", "--json"]).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Try to parse the JSON output
    if let Ok(bd_output) = serde_json::from_str::<BdLintOutput>(&stdout) {
        let results = convert_bd_lint_output(bd_output);
        return Ok(LintReport::from_results(results));
    }

    // If bd lint succeeded but didn't return JSON, check for text output
    if output.status.success() {
        if stdout.is_empty() || stdout.trim() == "All issues pass lint checks" {
            return Ok(LintReport::empty());
        }
        // Try to parse text output
        let results = parse_text_lint_output_all(&stdout);
        return Ok(LintReport::from_results(results));
    }

    Err(LintError::CliError(format!("bd lint failed: {}", stderr)))
}

/// Check if an issue has acceptance criteria
///
/// # Arguments
/// * `issue_id` - The ID of the issue to check
///
/// # Returns
/// * `Ok(true)` if the issue has acceptance criteria
/// * `Ok(false)` if the issue is missing acceptance criteria
/// * `Err(LintError)` if the check fails
pub fn check_acceptance_criteria(issue_id: &str) -> Result<bool, LintError> {
    let results = lint_issue(issue_id)?;

    // Check if any result is for missing acceptance criteria
    let has_missing_ac = results.iter().any(|r| {
        matches!(r.rule, LintRule::MissingAcceptanceCriteria)
            || r.message.to_lowercase().contains("acceptance criteria")
    });

    Ok(!has_missing_ac)
}

/// Get a suggestion for fixing a lint result
///
/// # Arguments
/// * `result` - The lint result to get a suggestion for
///
/// # Returns
/// A string with a suggestion for fixing the issue
pub fn suggest_fix(result: &LintResult) -> String {
    if let Some(suggestion) = &result.suggestion {
        return suggestion.clone();
    }

    match result.rule {
        LintRule::MissingDescription => {
            format!(
                "Add a description to issue {}: bd update {} --body \"<description>\"",
                result.issue_id, result.issue_id
            )
        }
        LintRule::MissingAcceptanceCriteria => {
            format!(
                "Add an 'Acceptance Criteria' section to issue {}: bd update {} --body \"$(bd show {} --body)\\n\\n## Acceptance Criteria\\n- [ ] Criterion 1\"",
                result.issue_id, result.issue_id, result.issue_id
            )
        }
        LintRule::MissingPriority => {
            format!(
                "Set priority for issue {}: bd update {} --priority <1-5>",
                result.issue_id, result.issue_id
            )
        }
        LintRule::OrphanedTask => {
            format!(
                "Link task {} to a parent epic: bd update {} --parent <epic-id>",
                result.issue_id, result.issue_id
            )
        }
        LintRule::CircularDependency => {
            format!(
                "Remove circular dependency from {}: bd dep rm {} <dependency-id>",
                result.issue_id, result.issue_id
            )
        }
        LintRule::StaleIssue => {
            format!(
                "Update issue {} with current status: bd comments add {} \"Status update: ...\"",
                result.issue_id, result.issue_id
            )
        }
        LintRule::MissingStepsToReproduce => {
            format!(
                "Add 'Steps to Reproduce' section to bug {}: bd update {} --body \"$(bd show {} --body)\\n\\n## Steps to Reproduce\\n1. Step one\\n2. Step two\"",
                result.issue_id, result.issue_id, result.issue_id
            )
        }
        LintRule::MissingSuccessCriteria => {
            format!(
                "Add 'Success Criteria' section to epic {}: bd update {} --body \"$(bd show {} --body)\\n\\n## Success Criteria\\n- Criterion 1\"",
                result.issue_id, result.issue_id, result.issue_id
            )
        }
        LintRule::MissingSection => {
            format!(
                "Add the missing section to issue {}: bd update {} --body \"...\"",
                result.issue_id, result.issue_id
            )
        }
    }
}

/// Convert bd lint JSON output to our LintResult format
fn convert_bd_lint_output(output: BdLintOutput) -> Vec<LintResult> {
    let mut results = Vec::new();

    for issue in output.issues {
        for section in &issue.missing_sections {
            let (rule, severity) =
                section_to_rule_and_severity(section, issue.issue_type.as_deref());

            let message = format!(
                "Issue {} is missing required section: {}",
                issue.id, section
            );

            let result = LintResult::new(&issue.id, rule, severity, &message);
            let suggestion = suggest_fix(&result);
            let result = result.with_suggestion(&suggestion);
            results.push(result);
        }
    }

    results
}

/// Map a section name to a lint rule and severity
fn section_to_rule_and_severity(
    section: &str,
    issue_type: Option<&str>,
) -> (LintRule, LintSeverity) {
    let section_lower = section.to_lowercase();

    if section_lower.contains("acceptance criteria") {
        return (LintRule::MissingAcceptanceCriteria, LintSeverity::Warning);
    }
    if section_lower.contains("steps to reproduce") {
        return (LintRule::MissingStepsToReproduce, LintSeverity::Warning);
    }
    if section_lower.contains("success criteria") {
        return (LintRule::MissingSuccessCriteria, LintSeverity::Warning);
    }

    // Default severity based on issue type
    let severity = match issue_type {
        Some("bug") => LintSeverity::Warning,
        Some("epic") => LintSeverity::Warning,
        Some("chore") => LintSeverity::Info,
        _ => LintSeverity::Warning,
    };

    (LintRule::MissingSection, severity)
}

/// Parse text output from bd lint for a single issue
fn parse_text_lint_output(output: &str, issue_id: &str) -> Result<Vec<LintResult>, LintError> {
    let mut results = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Look for patterns like "Missing: Acceptance Criteria" or "missing Acceptance Criteria"
        if line.to_lowercase().contains("missing") {
            let section = line
                .replace("Missing:", "")
                .replace("missing:", "")
                .replace("Missing", "")
                .replace("missing", "")
                .trim()
                .to_string();

            if !section.is_empty() {
                let (rule, severity) = section_to_rule_and_severity(&section, None);
                let message = format!("Issue {} is missing required section: {}", issue_id, section);
                let result = LintResult::new(issue_id, rule, severity, &message);
                let suggestion = suggest_fix(&result);
                let result = result.with_suggestion(&suggestion);
                results.push(result);
            }
        }
    }

    Ok(results)
}

/// Parse text output from bd lint for all issues
fn parse_text_lint_output_all(output: &str) -> Vec<LintResult> {
    let mut results = Vec::new();
    let mut current_issue_id: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Check if this line starts with an issue ID (e.g., "rb-123:" or "ralph-beads-123:")
        if let Some(colon_pos) = line.find(':') {
            let potential_id = &line[..colon_pos];
            // Simple heuristic: issue IDs typically have a dash and numbers
            if potential_id.contains('-') && potential_id.chars().any(|c| c.is_ascii_digit()) {
                current_issue_id = Some(potential_id.to_string());
                let rest = line[colon_pos + 1..].trim();
                if rest.to_lowercase().contains("missing") {
                    let section = rest
                        .replace("Missing:", "")
                        .replace("missing:", "")
                        .replace("Missing", "")
                        .replace("missing", "")
                        .trim()
                        .to_string();

                    if !section.is_empty() {
                        let (rule, severity) = section_to_rule_and_severity(&section, None);
                        let message = format!(
                            "Issue {} is missing required section: {}",
                            potential_id, section
                        );
                        let result = LintResult::new(potential_id, rule, severity, &message);
                        let suggestion = suggest_fix(&result);
                        let result = result.with_suggestion(&suggestion);
                        results.push(result);
                    }
                }
                continue;
            }
        }

        // Continue with current issue if set
        if let Some(ref issue_id) = current_issue_id {
            if line.to_lowercase().contains("missing") {
                let section = line
                    .replace("Missing:", "")
                    .replace("missing:", "")
                    .replace("Missing", "")
                    .replace("missing", "")
                    .trim()
                    .to_string();

                if !section.is_empty() {
                    let (rule, severity) = section_to_rule_and_severity(&section, None);
                    let message = format!(
                        "Issue {} is missing required section: {}",
                        issue_id, section
                    );
                    let result = LintResult::new(issue_id, rule, severity, &message);
                    let suggestion = suggest_fix(&result);
                    let result = result.with_suggestion(&suggestion);
                    results.push(result);
                }
            }
        }
    }

    results
}

/// Filter lint results by minimum severity
pub fn filter_by_severity(results: Vec<LintResult>, min_severity: LintSeverity) -> Vec<LintResult> {
    results
        .into_iter()
        .filter(|r| match min_severity {
            LintSeverity::Error => r.severity == LintSeverity::Error,
            LintSeverity::Warning => {
                r.severity == LintSeverity::Error || r.severity == LintSeverity::Warning
            }
            LintSeverity::Info => true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_severity_display() {
        assert_eq!(format!("{}", LintSeverity::Error), "error");
        assert_eq!(format!("{}", LintSeverity::Warning), "warning");
        assert_eq!(format!("{}", LintSeverity::Info), "info");
    }

    #[test]
    fn test_lint_rule_display() {
        assert_eq!(
            format!("{}", LintRule::MissingAcceptanceCriteria),
            "missing-acceptance-criteria"
        );
        assert_eq!(format!("{}", LintRule::OrphanedTask), "orphaned-task");
    }

    #[test]
    fn test_lint_result_new() {
        let result = LintResult::new(
            "test-123",
            LintRule::MissingDescription,
            LintSeverity::Error,
            "Test message",
        );

        assert_eq!(result.issue_id, "test-123");
        assert_eq!(result.rule, LintRule::MissingDescription);
        assert_eq!(result.severity, LintSeverity::Error);
        assert_eq!(result.message, "Test message");
        assert!(result.suggestion.is_none());
    }

    #[test]
    fn test_lint_result_with_suggestion() {
        let result = LintResult::new(
            "test-123",
            LintRule::MissingDescription,
            LintSeverity::Error,
            "Test message",
        )
        .with_suggestion("Add a description");

        assert_eq!(result.suggestion, Some("Add a description".to_string()));
    }

    #[test]
    fn test_lint_report_from_results_empty() {
        let report = LintReport::from_results(Vec::new());

        assert!(report.passed);
        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
        assert_eq!(report.infos, 0);
        assert!(report.summary.contains("passed"));
    }

    #[test]
    fn test_lint_report_from_results_with_errors() {
        let results = vec![
            LintResult::new(
                "a",
                LintRule::MissingDescription,
                LintSeverity::Error,
                "msg1",
            ),
            LintResult::new(
                "b",
                LintRule::MissingAcceptanceCriteria,
                LintSeverity::Warning,
                "msg2",
            ),
            LintResult::new("c", LintRule::StaleIssue, LintSeverity::Info, "msg3"),
        ];

        let report = LintReport::from_results(results);

        assert!(!report.passed);
        assert_eq!(report.errors, 1);
        assert_eq!(report.warnings, 1);
        assert_eq!(report.infos, 1);
        assert!(report.summary.contains("1 error"));
        assert!(report.summary.contains("1 warning"));
    }

    #[test]
    fn test_lint_report_from_results_only_infos() {
        let results = vec![LintResult::new(
            "a",
            LintRule::StaleIssue,
            LintSeverity::Info,
            "msg",
        )];

        let report = LintReport::from_results(results);

        assert!(report.passed);
        assert_eq!(report.infos, 1);
        assert!(report.summary.contains("info"));
    }

    #[test]
    fn test_lint_report_empty() {
        let report = LintReport::empty();

        assert!(report.passed);
        assert!(report.results.is_empty());
        assert_eq!(report.summary, "No issues found");
    }

    #[test]
    fn test_lint_report_merge() {
        let mut report1 = LintReport::from_results(vec![LintResult::new(
            "a",
            LintRule::MissingDescription,
            LintSeverity::Error,
            "msg1",
        )]);

        let report2 = LintReport::from_results(vec![LintResult::new(
            "b",
            LintRule::MissingAcceptanceCriteria,
            LintSeverity::Warning,
            "msg2",
        )]);

        report1.merge(report2);

        assert_eq!(report1.results.len(), 2);
        assert_eq!(report1.errors, 1);
        assert_eq!(report1.warnings, 1);
        assert!(!report1.passed);
    }

    #[test]
    fn test_suggest_fix_missing_description() {
        let result = LintResult::new(
            "test-123",
            LintRule::MissingDescription,
            LintSeverity::Error,
            "Missing description",
        );

        let suggestion = suggest_fix(&result);
        assert!(suggestion.contains("bd update test-123"));
        assert!(suggestion.contains("--body"));
    }

    #[test]
    fn test_suggest_fix_missing_acceptance_criteria() {
        let result = LintResult::new(
            "test-123",
            LintRule::MissingAcceptanceCriteria,
            LintSeverity::Warning,
            "Missing AC",
        );

        let suggestion = suggest_fix(&result);
        assert!(suggestion.contains("Acceptance Criteria"));
    }

    #[test]
    fn test_suggest_fix_orphaned_task() {
        let result = LintResult::new(
            "test-123",
            LintRule::OrphanedTask,
            LintSeverity::Warning,
            "Task without parent",
        );

        let suggestion = suggest_fix(&result);
        assert!(suggestion.contains("--parent"));
    }

    #[test]
    fn test_section_to_rule_acceptance_criteria() {
        let (rule, severity) = section_to_rule_and_severity("Acceptance Criteria", Some("task"));
        assert_eq!(rule, LintRule::MissingAcceptanceCriteria);
        assert_eq!(severity, LintSeverity::Warning);
    }

    #[test]
    fn test_section_to_rule_steps_to_reproduce() {
        let (rule, severity) = section_to_rule_and_severity("Steps to Reproduce", Some("bug"));
        assert_eq!(rule, LintRule::MissingStepsToReproduce);
        assert_eq!(severity, LintSeverity::Warning);
    }

    #[test]
    fn test_section_to_rule_success_criteria() {
        let (rule, severity) = section_to_rule_and_severity("Success Criteria", Some("epic"));
        assert_eq!(rule, LintRule::MissingSuccessCriteria);
        assert_eq!(severity, LintSeverity::Warning);
    }

    #[test]
    fn test_section_to_rule_chore_is_info() {
        let (rule, severity) = section_to_rule_and_severity("Some Section", Some("chore"));
        assert_eq!(rule, LintRule::MissingSection);
        assert_eq!(severity, LintSeverity::Info);
    }

    #[test]
    fn test_filter_by_severity_error() {
        let results = vec![
            LintResult::new(
                "a",
                LintRule::MissingDescription,
                LintSeverity::Error,
                "msg1",
            ),
            LintResult::new(
                "b",
                LintRule::MissingAcceptanceCriteria,
                LintSeverity::Warning,
                "msg2",
            ),
            LintResult::new("c", LintRule::StaleIssue, LintSeverity::Info, "msg3"),
        ];

        let filtered = filter_by_severity(results, LintSeverity::Error);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].issue_id, "a");
    }

    #[test]
    fn test_filter_by_severity_warning() {
        let results = vec![
            LintResult::new(
                "a",
                LintRule::MissingDescription,
                LintSeverity::Error,
                "msg1",
            ),
            LintResult::new(
                "b",
                LintRule::MissingAcceptanceCriteria,
                LintSeverity::Warning,
                "msg2",
            ),
            LintResult::new("c", LintRule::StaleIssue, LintSeverity::Info, "msg3"),
        ];

        let filtered = filter_by_severity(results, LintSeverity::Warning);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_by_severity_info() {
        let results = vec![
            LintResult::new(
                "a",
                LintRule::MissingDescription,
                LintSeverity::Error,
                "msg1",
            ),
            LintResult::new(
                "b",
                LintRule::MissingAcceptanceCriteria,
                LintSeverity::Warning,
                "msg2",
            ),
            LintResult::new("c", LintRule::StaleIssue, LintSeverity::Info, "msg3"),
        ];

        let filtered = filter_by_severity(results, LintSeverity::Info);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_lint_result_serialization() {
        let result = LintResult::new(
            "test-123",
            LintRule::MissingAcceptanceCriteria,
            LintSeverity::Warning,
            "Missing AC",
        )
        .with_suggestion("Add AC section");

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"issue_id\":\"test-123\""));
        assert!(json.contains("\"rule\":\"missing_acceptance_criteria\""));
        assert!(json.contains("\"severity\":\"warning\""));
        assert!(json.contains("\"suggestion\":\"Add AC section\""));
    }

    #[test]
    fn test_lint_result_serialization_no_suggestion() {
        let result = LintResult::new(
            "test-123",
            LintRule::MissingDescription,
            LintSeverity::Error,
            "Test",
        );

        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("suggestion"));
    }

    #[test]
    fn test_lint_report_serialization() {
        let results = vec![LintResult::new(
            "a",
            LintRule::MissingDescription,
            LintSeverity::Error,
            "msg",
        )];
        let report = LintReport::from_results(results);

        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("\"passed\": false"));
        assert!(json.contains("\"errors\": 1"));
        assert!(json.contains("\"warnings\": 0"));
    }
}
