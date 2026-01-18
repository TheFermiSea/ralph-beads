//! Gates Integration Module
//!
//! Wraps the beads CLI `bd gate` commands to provide type-safe gate management
//! for async coordination in ralph-beads workflows.
//!
//! Gates are async wait conditions that block workflow steps until resolved.
//! They support various types:
//! - Human: Requires manual approval via `bd gate resolve`
//! - Timer: Auto-expires after timeout duration
//! - GitHub Run (gh:run): Waits for GitHub Actions workflow completion
//! - GitHub PR (gh:pr): Waits for PR merge
//! - Bead: Waits for cross-rig bead to close

use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Command;
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during gate operations
#[derive(Error, Debug)]
pub enum GateError {
    #[error("Invalid gate type: {0}. Valid types: human, timer, gh:run, gh:pr, bead")]
    InvalidGateType(String),

    #[error("Invalid gate status: {0}. Valid statuses: pending, passed, failed, expired")]
    InvalidGateStatus(String),

    #[error("Gate not found: {0}")]
    NotFound(String),

    #[error("Beads CLI error: {0}")]
    CliError(String),

    #[error("Failed to execute bd command: {0}")]
    ExecutionError(#[from] std::io::Error),

    #[error("Failed to parse JSON output: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Gate operation timed out after {0} seconds")]
    Timeout(u64),

    #[error("Invalid duration format: {0}")]
    InvalidDuration(String),
}

/// Gate types supported by beads
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateType {
    /// Human approval gate - requires manual `bd gate resolve`
    Human,
    /// Timer gate - auto-expires after timeout duration
    Timer,
    /// GitHub Actions workflow run gate
    #[serde(rename = "gh:run")]
    GitHubRun,
    /// GitHub PR merge gate
    #[serde(rename = "gh:pr")]
    GitHubPr,
    /// Cross-rig bead gate - waits for another bead to close
    Bead,
}

impl fmt::Display for GateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateType::Human => write!(f, "human"),
            GateType::Timer => write!(f, "timer"),
            GateType::GitHubRun => write!(f, "gh:run"),
            GateType::GitHubPr => write!(f, "gh:pr"),
            GateType::Bead => write!(f, "bead"),
        }
    }
}

impl FromStr for GateType {
    type Err = GateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" | "approval" => Ok(GateType::Human),
            "timer" => Ok(GateType::Timer),
            "gh:run" | "github-run" | "github_run" | "ghrun" => Ok(GateType::GitHubRun),
            "gh:pr" | "github-pr" | "github_pr" | "ghpr" => Ok(GateType::GitHubPr),
            "bead" => Ok(GateType::Bead),
            _ => Err(GateError::InvalidGateType(s.to_string())),
        }
    }
}

/// Gate status - current state of a gate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GateStatus {
    /// Gate is waiting to be resolved
    #[default]
    Pending,
    /// Gate condition was satisfied (resolved successfully)
    Passed,
    /// Gate condition failed (e.g., CI failed, PR closed without merge)
    Failed,
    /// Timer gate expired without resolution
    Expired,
}

impl fmt::Display for GateStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateStatus::Pending => write!(f, "pending"),
            GateStatus::Passed => write!(f, "passed"),
            GateStatus::Failed => write!(f, "failed"),
            GateStatus::Expired => write!(f, "expired"),
        }
    }
}

impl FromStr for GateStatus {
    type Err = GateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" | "open" => Ok(GateStatus::Pending),
            "passed" | "resolved" | "closed" => Ok(GateStatus::Passed),
            "failed" | "failure" => Ok(GateStatus::Failed),
            "expired" | "timeout" => Ok(GateStatus::Expired),
            _ => Err(GateError::InvalidGateStatus(s.to_string())),
        }
    }
}

/// Configuration options for creating a gate
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateConfig {
    /// Timer duration (for timer gates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timer_duration: Option<String>,

    /// GitHub check name or run ID (for GitHub gates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_check: Option<String>,

    /// Target bead ID (for bead gates, format: "rig:bead-id")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub await_bead: Option<String>,

    /// Issue ID this gate is associated with
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_id: Option<String>,

    /// Optional title for the gate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl GateConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timer(mut self, duration: &str) -> Self {
        self.timer_duration = Some(duration.to_string());
        self
    }

    pub fn with_github_check(mut self, check: &str) -> Self {
        self.github_check = Some(check.to_string());
        self
    }

    pub fn with_await_bead(mut self, bead_ref: &str) -> Self {
        self.await_bead = Some(bead_ref.to_string());
        self
    }

    pub fn with_issue(mut self, issue_id: &str) -> Self {
        self.issue_id = Some(issue_id.to_string());
        self
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }
}

/// A gate issue from beads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gate {
    /// Gate issue ID
    pub id: String,

    /// Associated issue ID (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_id: Option<String>,

    /// Gate type
    pub gate_type: GateType,

    /// Current status
    pub status: GateStatus,

    /// Creation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Gate title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Await ID (e.g., GitHub run ID, bead reference)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub await_id: Option<String>,

    /// Waiters registered on this gate
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waiters: Vec<String>,
}

/// Result of a gate check operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheckResult {
    /// Gate ID
    pub gate_id: String,

    /// Previous status
    pub previous_status: GateStatus,

    /// New status after check
    pub new_status: GateStatus,

    /// Whether the gate was resolved by this check
    pub resolved: bool,

    /// Additional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Create a new gate
///
/// # Arguments
/// * `gate_type` - The type of gate to create
/// * `config` - Configuration options for the gate
///
/// # Returns
/// The ID of the created gate
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::gates::{create_gate, GateType, GateConfig};
///
/// // Create a human approval gate
/// let gate_id = create_gate(GateType::Human, GateConfig::new().with_title("Review required"))?;
///
/// // Create a timer gate that expires in 1 hour
/// let gate_id = create_gate(GateType::Timer, GateConfig::new().with_timer("1h"))?;
/// ```
pub fn create_gate(gate_type: GateType, config: GateConfig) -> Result<String, GateError> {
    let mut args = vec![
        "create".to_string(),
        "--type=gate".to_string(),
        "--silent".to_string(),
    ];

    // Add title
    let title = config.title.unwrap_or_else(|| {
        format!("Gate: {} approval", gate_type)
    });
    args.push(format!("--title={}", title));

    // Build description with gate type info
    let mut description_parts = vec![format!("await_type: {}", gate_type)];

    if let Some(desc) = &config.description {
        description_parts.push(desc.clone());
    }

    // Add type-specific configuration to description
    match gate_type {
        GateType::Timer => {
            if let Some(duration) = &config.timer_duration {
                description_parts.push(format!("timeout: {}", duration));
            }
        }
        GateType::GitHubRun | GateType::GitHubPr => {
            if let Some(check) = &config.github_check {
                description_parts.push(format!("await_id: {}", check));
            }
        }
        GateType::Bead => {
            if let Some(bead_ref) = &config.await_bead {
                description_parts.push(format!("await_id: {}", bead_ref));
            }
        }
        GateType::Human => {
            // Human gates don't need additional config
        }
    }

    args.push(format!("--description={}", description_parts.join("\n")));

    // Add parent issue if specified
    if let Some(issue_id) = &config.issue_id {
        args.push(format!("--parent={}", issue_id));
    }

    // Add gate type as label
    args.push(format!("--labels=gate:{}", gate_type));

    let output = Command::new("bd")
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GateError::CliError(stderr.to_string()));
    }

    let gate_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // If we have an await_id (for gh:run, gh:pr, bead), set it
    if let Some(await_id) = config.github_check.or(config.await_bead) {
        let _ = Command::new("bd")
            .args(["update", &gate_id, &format!("--await-id={}", await_id)])
            .output();
    }

    Ok(gate_id)
}

/// Check the status of a gate
///
/// # Arguments
/// * `gate_id` - The gate ID to check
///
/// # Returns
/// The current status of the gate
pub fn check_gate(gate_id: &str) -> Result<GateStatus, GateError> {
    let output = Command::new("bd")
        .args(["gate", "show", gate_id, "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(GateError::NotFound(gate_id.to_string()));
        }
        return Err(GateError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let gate: serde_json::Value = serde_json::from_str(&stdout)?;

    // Check status field - beads uses "status" with values like "open", "closed"
    let status_str = gate.get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("pending");

    // Map beads status to our GateStatus
    let status = match status_str {
        "closed" => GateStatus::Passed,
        "open" => GateStatus::Pending,
        "blocked" => GateStatus::Failed,
        _ => status_str.parse().unwrap_or(GateStatus::Pending),
    };

    Ok(status)
}

/// Approve/resolve a gate manually
///
/// This is typically used for human approval gates.
///
/// # Arguments
/// * `gate_id` - The gate ID to approve
/// * `reason` - Optional reason for approval
pub fn approve_gate(gate_id: &str, reason: Option<&str>) -> Result<(), GateError> {
    let mut args = vec!["gate", "resolve", gate_id];

    let reason_string;
    if let Some(r) = reason {
        reason_string = format!("--reason={}", r);
        args.push(&reason_string);
    }

    let output = Command::new("bd")
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(GateError::NotFound(gate_id.to_string()));
        }
        return Err(GateError::CliError(stderr.to_string()));
    }

    Ok(())
}

/// List gates with optional filtering
///
/// # Arguments
/// * `issue_id` - Optional issue ID to filter gates by (parent relationship)
/// * `include_closed` - Whether to include closed gates
///
/// # Returns
/// Vector of gates matching the filter
pub fn list_gates(issue_id: Option<&str>, include_closed: bool) -> Result<Vec<Gate>, GateError> {
    let mut args = vec!["gate", "list", "--json"];

    if include_closed {
        args.push("--all");
    }

    let output = Command::new("bd")
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GateError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Handle empty result
    if stdout.trim().is_empty() || stdout.trim() == "[]" {
        return Ok(Vec::new());
    }

    let gates_json: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;

    let mut gates = Vec::new();
    for gate_json in gates_json {
        let gate = parse_gate_from_json(&gate_json)?;

        // Filter by issue_id if specified
        if let Some(filter_id) = issue_id {
            if gate.issue_id.as_ref() != Some(&filter_id.to_string()) {
                continue;
            }
        }

        gates.push(gate);
    }

    Ok(gates)
}

/// Get detailed information about a specific gate
///
/// # Arguments
/// * `gate_id` - The gate ID to retrieve
///
/// # Returns
/// The gate details
pub fn get_gate(gate_id: &str) -> Result<Gate, GateError> {
    let output = Command::new("bd")
        .args(["gate", "show", gate_id, "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(GateError::NotFound(gate_id.to_string()));
        }
        return Err(GateError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let gate_json: serde_json::Value = serde_json::from_str(&stdout)?;

    parse_gate_from_json(&gate_json)
}

/// Wait for a gate to be resolved
///
/// Polls the gate status until it's no longer pending or timeout is reached.
///
/// # Arguments
/// * `gate_id` - The gate ID to wait for
/// * `timeout` - Maximum time to wait
/// * `poll_interval` - Time between status checks (default 5 seconds)
///
/// # Returns
/// The final gate status when resolved or timeout
pub fn wait_for_gate(
    gate_id: &str,
    timeout: Duration,
    poll_interval: Option<Duration>,
) -> Result<GateStatus, GateError> {
    let interval = poll_interval.unwrap_or(Duration::from_secs(5));
    let start = std::time::Instant::now();

    loop {
        // Run gate check to evaluate conditions
        let _ = Command::new("bd")
            .args(["gate", "check"])
            .output();

        let status = check_gate(gate_id)?;

        if status != GateStatus::Pending {
            return Ok(status);
        }

        if start.elapsed() >= timeout {
            return Err(GateError::Timeout(timeout.as_secs()));
        }

        std::thread::sleep(interval);
    }
}

/// Evaluate all open gates and auto-close resolved ones
///
/// # Arguments
/// * `gate_type` - Optional gate type filter
/// * `dry_run` - If true, show what would happen without making changes
///
/// # Returns
/// Results of the gate check operation
pub fn evaluate_gates(
    gate_type: Option<GateType>,
    dry_run: bool,
) -> Result<Vec<GateCheckResult>, GateError> {
    let mut args = vec!["gate", "check", "--json"];

    let type_string;
    if let Some(gt) = gate_type {
        type_string = format!("--type={}", gt);
        args.push(&type_string);
    }

    if dry_run {
        args.push("--dry-run");
    }

    let output = Command::new("bd")
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GateError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Handle empty result
    if stdout.trim().is_empty() || stdout.trim() == "[]" {
        return Ok(Vec::new());
    }

    // Try to parse as array of results
    let results: Vec<GateCheckResult> = serde_json::from_str(&stdout).unwrap_or_default();
    Ok(results)
}

/// Add a waiter to a gate
///
/// When the gate closes, the waiter will receive a wake notification.
///
/// # Arguments
/// * `gate_id` - The gate ID
/// * `waiter` - The waiter address (e.g., "rig/polecats/Name")
pub fn add_waiter(gate_id: &str, waiter: &str) -> Result<(), GateError> {
    let output = Command::new("bd")
        .args(["gate", "add-waiter", gate_id, waiter])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(GateError::NotFound(gate_id.to_string()));
        }
        return Err(GateError::CliError(stderr.to_string()));
    }

    Ok(())
}

/// Parse a gate from JSON value returned by beads
fn parse_gate_from_json(json: &serde_json::Value) -> Result<Gate, GateError> {
    let id = json.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Parse gate type from labels or description
    let gate_type = extract_gate_type(json);

    // Parse status
    let status_str = json.get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("open");

    let status = match status_str {
        "closed" => GateStatus::Passed,
        "open" => GateStatus::Pending,
        "blocked" => GateStatus::Failed,
        _ => GateStatus::Pending,
    };

    let issue_id = json.get("parent")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let created_at = json.get("created_at")
        .or_else(|| json.get("createdAt"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let title = json.get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let await_id = json.get("await_id")
        .or_else(|| json.get("awaitId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let waiters = json.get("waiters")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect())
        .unwrap_or_default();

    Ok(Gate {
        id,
        issue_id,
        gate_type,
        status,
        created_at,
        title,
        await_id,
        waiters,
    })
}

/// Extract gate type from JSON (checks labels and description)
fn extract_gate_type(json: &serde_json::Value) -> GateType {
    // Check labels for gate type
    if let Some(labels) = json.get("labels").and_then(|v| v.as_array()) {
        for label in labels {
            if let Some(label_str) = label.as_str() {
                if label_str.starts_with("gate:") {
                    if let Ok(gt) = label_str.trim_start_matches("gate:").parse() {
                        return gt;
                    }
                }
            }
        }
    }

    // Check description for await_type
    if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
        for line in desc.lines() {
            if line.starts_with("await_type:") {
                if let Ok(gt) = line.trim_start_matches("await_type:").trim().parse() {
                    return gt;
                }
            }
        }
    }

    // Check await_type field directly
    if let Some(await_type) = json.get("await_type").and_then(|v| v.as_str()) {
        if let Ok(gt) = await_type.parse() {
            return gt;
        }
    }

    // Default to human
    GateType::Human
}

/// Parse a duration string (e.g., "5m", "1h", "30s") into Duration
pub fn parse_duration(s: &str) -> Result<Duration, GateError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(GateError::InvalidDuration("empty duration".to_string()));
    }

    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len()-2], "ms")
    } else if s.ends_with('s') || s.ends_with('m') || s.ends_with('h') || s.ends_with('d') {
        (&s[..s.len()-1], &s[s.len()-1..])
    } else {
        // Assume seconds if no unit
        (s, "s")
    };

    let num: u64 = num_str.parse()
        .map_err(|_| GateError::InvalidDuration(s.to_string()))?;

    let duration = match unit {
        "ms" => Duration::from_millis(num),
        "s" => Duration::from_secs(num),
        "m" => Duration::from_secs(num * 60),
        "h" => Duration::from_secs(num * 3600),
        "d" => Duration::from_secs(num * 86400),
        _ => return Err(GateError::InvalidDuration(s.to_string())),
    };

    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_type_from_str() {
        assert_eq!(GateType::from_str("human").unwrap(), GateType::Human);
        assert_eq!(GateType::from_str("approval").unwrap(), GateType::Human);
        assert_eq!(GateType::from_str("HUMAN").unwrap(), GateType::Human);
        assert_eq!(GateType::from_str("timer").unwrap(), GateType::Timer);
        assert_eq!(GateType::from_str("gh:run").unwrap(), GateType::GitHubRun);
        assert_eq!(GateType::from_str("github-run").unwrap(), GateType::GitHubRun);
        assert_eq!(GateType::from_str("gh:pr").unwrap(), GateType::GitHubPr);
        assert_eq!(GateType::from_str("github-pr").unwrap(), GateType::GitHubPr);
        assert_eq!(GateType::from_str("bead").unwrap(), GateType::Bead);
    }

    #[test]
    fn test_gate_type_from_str_invalid() {
        assert!(GateType::from_str("invalid").is_err());
        assert!(GateType::from_str("").is_err());
    }

    #[test]
    fn test_gate_type_display() {
        assert_eq!(GateType::Human.to_string(), "human");
        assert_eq!(GateType::Timer.to_string(), "timer");
        assert_eq!(GateType::GitHubRun.to_string(), "gh:run");
        assert_eq!(GateType::GitHubPr.to_string(), "gh:pr");
        assert_eq!(GateType::Bead.to_string(), "bead");
    }

    #[test]
    fn test_gate_status_from_str() {
        assert_eq!(GateStatus::from_str("pending").unwrap(), GateStatus::Pending);
        assert_eq!(GateStatus::from_str("open").unwrap(), GateStatus::Pending);
        assert_eq!(GateStatus::from_str("passed").unwrap(), GateStatus::Passed);
        assert_eq!(GateStatus::from_str("resolved").unwrap(), GateStatus::Passed);
        assert_eq!(GateStatus::from_str("closed").unwrap(), GateStatus::Passed);
        assert_eq!(GateStatus::from_str("failed").unwrap(), GateStatus::Failed);
        assert_eq!(GateStatus::from_str("failure").unwrap(), GateStatus::Failed);
        assert_eq!(GateStatus::from_str("expired").unwrap(), GateStatus::Expired);
        assert_eq!(GateStatus::from_str("timeout").unwrap(), GateStatus::Expired);
    }

    #[test]
    fn test_gate_status_from_str_invalid() {
        assert!(GateStatus::from_str("invalid").is_err());
        assert!(GateStatus::from_str("").is_err());
    }

    #[test]
    fn test_gate_status_display() {
        assert_eq!(GateStatus::Pending.to_string(), "pending");
        assert_eq!(GateStatus::Passed.to_string(), "passed");
        assert_eq!(GateStatus::Failed.to_string(), "failed");
        assert_eq!(GateStatus::Expired.to_string(), "expired");
    }

    #[test]
    fn test_gate_config_builder() {
        let config = GateConfig::new()
            .with_title("Test Gate")
            .with_timer("1h")
            .with_issue("test-123");

        assert_eq!(config.title, Some("Test Gate".to_string()));
        assert_eq!(config.timer_duration, Some("1h".to_string()));
        assert_eq!(config.issue_id, Some("test-123".to_string()));
    }

    #[test]
    fn test_gate_config_github() {
        let config = GateConfig::new()
            .with_github_check("12345");

        assert_eq!(config.github_check, Some("12345".to_string()));
    }

    #[test]
    fn test_gate_config_bead() {
        let config = GateConfig::new()
            .with_await_bead("gastown:gt-abc123");

        assert_eq!(config.await_bead, Some("gastown:gt-abc123".to_string()));
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("2d").unwrap(), Duration::from_secs(172800));
        assert_eq!(parse_duration("100ms").unwrap(), Duration::from_millis(100));
        assert_eq!(parse_duration("60").unwrap(), Duration::from_secs(60)); // Default to seconds
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("5x").is_err());
    }

    #[test]
    fn test_gate_type_serialization() {
        let gate_type = GateType::GitHubRun;
        let json = serde_json::to_string(&gate_type).unwrap();
        assert_eq!(json, "\"gh:run\"");

        let deserialized: GateType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, GateType::GitHubRun);
    }

    #[test]
    fn test_gate_status_serialization() {
        let status = GateStatus::Passed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"passed\"");

        let deserialized: GateStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, GateStatus::Passed);
    }

    #[test]
    fn test_gate_serialization() {
        let gate = Gate {
            id: "gate-123".to_string(),
            issue_id: Some("task-456".to_string()),
            gate_type: GateType::Human,
            status: GateStatus::Pending,
            created_at: Some("2024-01-15T10:00:00Z".to_string()),
            title: Some("Review required".to_string()),
            await_id: None,
            waiters: vec!["waiter-1".to_string()],
        };

        let json = serde_json::to_string(&gate).unwrap();
        assert!(json.contains("\"id\":\"gate-123\""));
        assert!(json.contains("\"gate_type\":\"human\""));
        assert!(json.contains("\"status\":\"pending\""));
    }

    #[test]
    fn test_default_values() {
        assert_eq!(GateStatus::default(), GateStatus::Pending);

        let config = GateConfig::default();
        assert!(config.timer_duration.is_none());
        assert!(config.github_check.is_none());
    }

    #[test]
    fn test_extract_gate_type_from_label() {
        let json = serde_json::json!({
            "id": "test",
            "labels": ["gate:timer", "other"]
        });
        assert_eq!(extract_gate_type(&json), GateType::Timer);
    }

    #[test]
    fn test_extract_gate_type_from_description() {
        let json = serde_json::json!({
            "id": "test",
            "description": "await_type: gh:run\nother stuff"
        });
        assert_eq!(extract_gate_type(&json), GateType::GitHubRun);
    }

    #[test]
    fn test_extract_gate_type_default() {
        let json = serde_json::json!({
            "id": "test"
        });
        assert_eq!(extract_gate_type(&json), GateType::Human);
    }
}
