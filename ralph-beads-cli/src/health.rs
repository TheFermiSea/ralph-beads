//! Health Check System
//!
//! Provides pre-execution diagnostics to identify potential issues
//! before starting a workflow. Inspired by Context-Engine's
//! context compilation strategy.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Health check result status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// All checks passed
    Healthy,
    /// Some warnings but can proceed
    Warning,
    /// Issues that should be addressed
    Degraded,
    /// Critical issues - should not proceed
    Critical,
}

impl HealthStatus {
    /// Combine two statuses (takes the worse)
    pub fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Critical, _) | (_, Self::Critical) => Self::Critical,
            (Self::Degraded, _) | (_, Self::Degraded) => Self::Degraded,
            (Self::Warning, _) | (_, Self::Warning) => Self::Warning,
            _ => Self::Healthy,
        }
    }
}

/// Individual health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Check name
    pub name: String,
    /// Status of this check
    pub status: HealthStatus,
    /// Human-readable message
    pub message: String,
    /// Suggested fix (if not healthy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// Additional details
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, String>,
}

impl CheckResult {
    fn healthy(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Healthy,
            message: message.to_string(),
            fix: None,
            details: HashMap::new(),
        }
    }

    fn warning(name: &str, message: &str, fix: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Warning,
            message: message.to_string(),
            fix: Some(fix.to_string()),
            details: HashMap::new(),
        }
    }

    fn degraded(name: &str, message: &str, fix: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Degraded,
            message: message.to_string(),
            fix: Some(fix.to_string()),
            details: HashMap::new(),
        }
    }

    fn critical(name: &str, message: &str, fix: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Critical,
            message: message.to_string(),
            fix: Some(fix.to_string()),
            details: HashMap::new(),
        }
    }

    fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.insert(key.to_string(), value.to_string());
        self
    }
}

/// Complete health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall status
    pub status: HealthStatus,
    /// Individual check results
    pub checks: Vec<CheckResult>,
    /// Summary message
    pub summary: String,
    /// Whether it's safe to proceed
    pub can_proceed: bool,
}

impl HealthReport {
    /// Create report from check results
    pub fn from_checks(checks: Vec<CheckResult>) -> Self {
        let status = checks
            .iter()
            .map(|c| c.status)
            .fold(HealthStatus::Healthy, |a, b| a.combine(b));

        let can_proceed = !matches!(status, HealthStatus::Critical);

        let summary = match status {
            HealthStatus::Healthy => "All health checks passed".to_string(),
            HealthStatus::Warning => format!(
                "{} warning(s) found, can proceed with caution",
                checks
                    .iter()
                    .filter(|c| c.status == HealthStatus::Warning)
                    .count()
            ),
            HealthStatus::Degraded => format!(
                "{} issue(s) found that should be addressed",
                checks
                    .iter()
                    .filter(|c| c.status == HealthStatus::Degraded)
                    .count()
            ),
            HealthStatus::Critical => format!(
                "{} critical issue(s) - cannot proceed",
                checks
                    .iter()
                    .filter(|c| c.status == HealthStatus::Critical)
                    .count()
            ),
        };

        Self {
            status,
            checks,
            summary,
            can_proceed,
        }
    }
}

/// Health checker for Ralph-Beads
pub struct HealthChecker {
    /// Project directory
    project_dir: String,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(project_dir: &str) -> Self {
        Self {
            project_dir: project_dir.to_string(),
        }
    }

    /// Run all health checks
    pub fn check_all(&self) -> HealthReport {
        let mut checks = Vec::new();

        // Core checks
        checks.push(self.check_git());
        checks.push(self.check_beads());
        checks.push(self.check_working_directory());
        checks.push(self.check_git_status());

        // Framework-specific checks
        if Path::new(&self.project_dir).join("Cargo.toml").exists() {
            checks.push(self.check_rust());
        }
        if Path::new(&self.project_dir).join("package.json").exists() {
            checks.push(self.check_node());
        }
        if Path::new(&self.project_dir).join("pyproject.toml").exists()
            || Path::new(&self.project_dir).join("setup.py").exists()
        {
            checks.push(self.check_python());
        }

        // Environment checks
        checks.push(self.check_disk_space());
        checks.push(self.check_rust_cli());

        HealthReport::from_checks(checks)
    }

    /// Check if git is available and repo is valid
    fn check_git(&self) -> CheckResult {
        // Check git is installed
        if !command_exists("git") {
            return CheckResult::critical(
                "git",
                "Git is not installed",
                "Install git: https://git-scm.com/downloads",
            );
        }

        // Check if in a git repo
        let output = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                CheckResult::healthy("git", "Git repository detected")
            }
            _ => CheckResult::critical("git", "Not a git repository", "Initialize with: git init"),
        }
    }

    /// Check if beads is available and initialized
    fn check_beads(&self) -> CheckResult {
        // Check bd is installed
        if !command_exists("bd") {
            return CheckResult::degraded(
                "beads",
                "Beads CLI (bd) is not installed",
                "Install from: https://github.com/steveyegge/beads",
            );
        }

        // Check if beads is initialized
        let beads_dir = Path::new(&self.project_dir).join(".beads");
        if !beads_dir.exists() {
            return CheckResult::degraded(
                "beads",
                "Beads not initialized in this project",
                "Initialize with: bd init",
            );
        }

        // Check beads info
        let output = Command::new("bd")
            .args(["info", "--json"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                CheckResult::healthy("beads", "Beads initialized and functional")
            }
            _ => CheckResult::warning(
                "beads",
                "Beads initialized but may have issues",
                "Run: bd doctor",
            ),
        }
    }

    /// Check working directory is valid
    fn check_working_directory(&self) -> CheckResult {
        let path = Path::new(&self.project_dir);

        if !path.exists() {
            return CheckResult::critical(
                "directory",
                &format!("Project directory does not exist: {}", self.project_dir),
                "Verify the path is correct",
            );
        }

        if !path.is_dir() {
            return CheckResult::critical(
                "directory",
                &format!("Path is not a directory: {}", self.project_dir),
                "Provide a directory path",
            );
        }

        // Check write permissions
        let test_file = path.join(".ralph-health-check-test");
        match std::fs::write(&test_file, "test") {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                CheckResult::healthy("directory", "Project directory is writable")
            }
            Err(_) => CheckResult::critical(
                "directory",
                "Project directory is not writable",
                "Check permissions on the directory",
            ),
        }
    }

    /// Check git working tree status
    fn check_git_status(&self) -> CheckResult {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let status = String::from_utf8_lossy(&out.stdout);
                let changes = status.lines().count();

                if changes == 0 {
                    CheckResult::healthy("git_status", "Working tree is clean")
                } else if changes < 10 {
                    CheckResult::warning(
                        "git_status",
                        &format!("{} uncommitted changes", changes),
                        "Consider committing or stashing changes before starting",
                    )
                    .with_detail("changes", &changes.to_string())
                } else {
                    CheckResult::warning(
                        "git_status",
                        &format!("{} uncommitted changes (many)", changes),
                        "Commit or stash changes to avoid conflicts",
                    )
                    .with_detail("changes", &changes.to_string())
                }
            }
            _ => CheckResult::warning(
                "git_status",
                "Could not check git status",
                "Run: git status",
            ),
        }
    }

    /// Check Rust toolchain
    fn check_rust(&self) -> CheckResult {
        if !command_exists("cargo") {
            return CheckResult::degraded(
                "rust",
                "Cargo not found",
                "Install Rust: https://rustup.rs",
            );
        }

        // Check if project builds
        let output = Command::new("cargo")
            .args(["check", "--message-format=short"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                // Check for warnings
                let stderr = String::from_utf8_lossy(&out.stderr);
                let warnings = stderr.matches("warning:").count();

                if warnings == 0 {
                    CheckResult::healthy("rust", "Rust project compiles cleanly")
                } else {
                    CheckResult::warning(
                        "rust",
                        &format!("Rust project compiles with {} warnings", warnings),
                        "Run: cargo clippy --fix",
                    )
                    .with_detail("warnings", &warnings.to_string())
                }
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let errors = stderr.matches("error").count();
                CheckResult::degraded(
                    "rust",
                    &format!("Rust project has {} compile errors", errors),
                    "Fix compile errors before proceeding",
                )
                .with_detail("errors", &errors.to_string())
            }
            Err(_) => CheckResult::warning(
                "rust",
                "Could not run cargo check",
                "Verify Cargo.toml is valid",
            ),
        }
    }

    /// Check Node.js environment
    fn check_node(&self) -> CheckResult {
        if !command_exists("node") {
            return CheckResult::degraded(
                "node",
                "Node.js not found",
                "Install Node.js: https://nodejs.org",
            );
        }

        // Check if node_modules exists
        let node_modules = Path::new(&self.project_dir).join("node_modules");
        if !node_modules.exists() {
            return CheckResult::warning("node", "node_modules not found", "Run: npm install");
        }

        CheckResult::healthy("node", "Node.js environment ready")
    }

    /// Check Python environment
    fn check_python(&self) -> CheckResult {
        if !command_exists("python") && !command_exists("python3") {
            return CheckResult::degraded(
                "python",
                "Python not found",
                "Install Python: https://python.org",
            );
        }

        // Check for virtual environment
        let venv = Path::new(&self.project_dir).join(".venv");
        let venv_alt = Path::new(&self.project_dir).join("venv");

        if !venv.exists() && !venv_alt.exists() {
            return CheckResult::warning(
                "python",
                "No virtual environment found",
                "Create with: python -m venv .venv",
            );
        }

        CheckResult::healthy("python", "Python environment ready")
    }

    /// Check available disk space
    fn check_disk_space(&self) -> CheckResult {
        // Simple heuristic - try to get disk space
        #[cfg(unix)]
        {
            let output = Command::new("df").args(["-h", &self.project_dir]).output();

            if let Ok(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Parse df output (very rough)
                if stdout.contains("100%") {
                    return CheckResult::critical(
                        "disk",
                        "Disk is full",
                        "Free up disk space before proceeding",
                    );
                }
                if stdout.contains("9") && stdout.contains("%") {
                    // Rough check for 90%+
                    return CheckResult::warning(
                        "disk",
                        "Disk space is low",
                        "Consider freeing up space",
                    );
                }
            }
        }

        CheckResult::healthy("disk", "Disk space appears adequate")
    }

    /// Check if ralph-beads-cli is available
    fn check_rust_cli(&self) -> CheckResult {
        if command_exists("ralph-beads-cli") {
            CheckResult::healthy(
                "ralph_cli",
                "ralph-beads-cli is available (Rust acceleration)",
            )
        } else {
            CheckResult::warning(
                "ralph_cli",
                "ralph-beads-cli not found",
                "Build with: cd ralph-beads-cli && cargo build --release",
            )
        }
    }
}

/// Check if a command exists in PATH
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_combine() {
        assert_eq!(
            HealthStatus::Healthy.combine(HealthStatus::Healthy),
            HealthStatus::Healthy
        );
        assert_eq!(
            HealthStatus::Healthy.combine(HealthStatus::Warning),
            HealthStatus::Warning
        );
        assert_eq!(
            HealthStatus::Warning.combine(HealthStatus::Critical),
            HealthStatus::Critical
        );
    }

    #[test]
    fn test_health_report_from_checks() {
        let checks = vec![
            CheckResult::healthy("test1", "OK"),
            CheckResult::warning("test2", "Minor issue", "Fix it"),
        ];

        let report = HealthReport::from_checks(checks);
        assert_eq!(report.status, HealthStatus::Warning);
        assert!(report.can_proceed);
    }

    #[test]
    fn test_critical_blocks_proceed() {
        let checks = vec![
            CheckResult::healthy("test1", "OK"),
            CheckResult::critical("test2", "Bad", "Fix it"),
        ];

        let report = HealthReport::from_checks(checks);
        assert_eq!(report.status, HealthStatus::Critical);
        assert!(!report.can_proceed);
    }

    #[test]
    fn test_check_result_with_details() {
        let result = CheckResult::warning("test", "Message", "Fix")
            .with_detail("count", "5")
            .with_detail("type", "error");

        assert_eq!(result.details.len(), 2);
        assert_eq!(result.details.get("count"), Some(&"5".to_string()));
    }
}
