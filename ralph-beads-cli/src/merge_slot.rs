//! Merge Slot Integration Module
//!
//! Wraps the beads CLI `bd merge-slot` commands to provide type-safe merge slot
//! management for multi-agent serialization in ralph-beads workflows.
//!
//! Merge slots are exclusive access primitives that prevent "monkey knife fights"
//! where multiple agents race to resolve conflicts and create cascading conflicts.
//! Only one agent can hold a merge slot at a time, serializing conflict resolution.
//!
//! Each rig has one merge slot bead: `<prefix>-merge-slot` (labeled `gt:slot`).
//! The slot uses:
//! - status=open: slot is available
//! - status=in_progress: slot is held
//! - holder field: who currently holds the slot
//! - waiters field: priority-ordered queue of waiters

use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Command;
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Errors that can occur during merge slot operations
#[derive(Error, Debug)]
pub enum MergeSlotError {
    #[error("Invalid slot status: {0}. Valid statuses: available, held, expired")]
    InvalidSlotStatus(String),

    #[error("Merge slot not found for this rig")]
    NotFound,

    #[error("Slot is currently held by: {0}")]
    SlotHeld(String),

    #[error("Slot acquisition timed out after {0} seconds")]
    Timeout(u64),

    #[error("Not the current holder - cannot release slot")]
    NotHolder,

    #[error("Beads CLI error: {0}")]
    CliError(String),

    #[error("Failed to execute bd command: {0}")]
    ExecutionError(#[from] std::io::Error),

    #[error("Failed to parse JSON output: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Status of a merge slot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SlotStatus {
    /// Slot is available for acquisition
    #[default]
    Available,
    /// Slot is currently held by an agent
    Held,
    /// Slot has expired (holder timeout)
    Expired,
}

impl fmt::Display for SlotStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotStatus::Available => write!(f, "available"),
            SlotStatus::Held => write!(f, "held"),
            SlotStatus::Expired => write!(f, "expired"),
        }
    }
}

impl FromStr for SlotStatus {
    type Err = MergeSlotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "available" | "open" => Ok(SlotStatus::Available),
            "held" | "in_progress" => Ok(SlotStatus::Held),
            "expired" | "timeout" => Ok(SlotStatus::Expired),
            _ => Err(MergeSlotError::InvalidSlotStatus(s.to_string())),
        }
    }
}

/// A merge slot representing exclusive access for conflict resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeSlot {
    /// Slot ID (usually `<prefix>-merge-slot`)
    pub slot_id: String,

    /// Current holder (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder: Option<String>,

    /// When the slot was acquired
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquired_at: Option<String>,

    /// When the slot expires (if timeout set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,

    /// Current status of the slot
    pub status: SlotStatus,

    /// List of waiters in priority order
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waiters: Vec<String>,
}

impl Default for MergeSlot {
    fn default() -> Self {
        Self {
            slot_id: String::new(),
            holder: None,
            acquired_at: None,
            expires_at: None,
            status: SlotStatus::Available,
            waiters: Vec::new(),
        }
    }
}

/// Configuration for slot operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlotConfig {
    /// Timeout in seconds for slot acquisition wait
    #[serde(default)]
    pub timeout_seconds: u64,

    /// Branch context (for future use)
    #[serde(default)]
    pub branch: String,

    /// Force release even if not holder
    #[serde(default)]
    pub force: bool,

    /// Add to waiters list if slot is held
    #[serde(default)]
    pub wait_flag: bool,
}

impl SlotConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    pub fn with_branch(mut self, branch: &str) -> Self {
        self.branch = branch.to_string();
        self
    }

    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    pub fn with_wait(mut self, wait: bool) -> Self {
        self.wait_flag = wait;
        self
    }
}

/// Acquire a merge slot for exclusive access
///
/// If the slot is available, it will be acquired and the holder field set.
/// If the slot is held, returns an error indicating who holds it.
///
/// # Arguments
/// * `branch` - Branch context (currently unused, for future expansion)
/// * `holder` - Identifier of the agent acquiring the slot
/// * `config` - Configuration options for acquisition
///
/// # Returns
/// The acquired merge slot on success
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::merge_slot::{acquire_slot, SlotConfig};
///
/// let slot = acquire_slot("main", "agent-1", SlotConfig::new())?;
/// println!("Acquired slot: {}", slot.slot_id);
/// ```
pub fn acquire_slot(
    _branch: &str,
    holder: &str,
    config: SlotConfig,
) -> Result<MergeSlot, MergeSlotError> {
    let mut args = vec!["merge-slot", "acquire", "--json"];

    let holder_arg = format!("--holder={}", holder);
    args.push(&holder_arg);

    if config.wait_flag {
        args.push("--wait");
    }

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("not found") || stderr.contains("no merge slot") {
            return Err(MergeSlotError::NotFound);
        }
        if stderr.contains("held") || stderr.contains("in_progress") {
            // Extract holder from error message if possible
            let held_by =
                extract_holder_from_error(&stderr).unwrap_or_else(|| "unknown".to_string());
            return Err(MergeSlotError::SlotHeld(held_by));
        }

        return Err(MergeSlotError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_slot_from_output(&stdout)
}

/// Release a merge slot after conflict resolution is complete
///
/// Sets status back to open and clears the holder field.
/// If there are waiters, they can then attempt to acquire.
///
/// # Arguments
/// * `branch` - Branch context (currently unused)
/// * `holder` - Identifier of the agent releasing (for verification)
///
/// # Returns
/// Ok(()) on success
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::merge_slot::release_slot;
///
/// release_slot("main", "agent-1")?;
/// ```
pub fn release_slot(_branch: &str, holder: &str) -> Result<(), MergeSlotError> {
    let holder_arg = format!("--holder={}", holder);
    let args = vec!["merge-slot", "release", &holder_arg];

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("not found") || stderr.contains("no merge slot") {
            return Err(MergeSlotError::NotFound);
        }
        if stderr.contains("not the holder") || stderr.contains("different holder") {
            return Err(MergeSlotError::NotHolder);
        }

        return Err(MergeSlotError::CliError(stderr.to_string()));
    }

    Ok(())
}

/// Check the status of a merge slot
///
/// Returns the current state of the merge slot without modifying it.
///
/// # Arguments
/// * `branch` - Branch context (currently unused)
///
/// # Returns
/// The current merge slot state
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::merge_slot::check_slot;
///
/// let slot = check_slot("main")?;
/// match slot.status {
///     SlotStatus::Available => println!("Slot is available"),
///     SlotStatus::Held => println!("Slot held by: {:?}", slot.holder),
///     SlotStatus::Expired => println!("Slot expired"),
/// }
/// ```
pub fn check_slot(_branch: &str) -> Result<MergeSlot, MergeSlotError> {
    let args = vec!["merge-slot", "check", "--json"];

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("not found") || stderr.contains("no merge slot") {
            return Err(MergeSlotError::NotFound);
        }

        return Err(MergeSlotError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_slot_from_output(&stdout)
}

/// Wait for a slot to become available and acquire it
///
/// Polls the slot status until it becomes available or timeout is reached.
/// Once available, attempts to acquire the slot.
///
/// # Arguments
/// * `branch` - Branch context
/// * `holder` - Identifier of the agent acquiring
/// * `timeout` - Maximum time to wait in seconds
///
/// # Returns
/// The acquired merge slot on success
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::merge_slot::wait_for_slot;
///
/// // Wait up to 5 minutes for the slot
/// let slot = wait_for_slot("main", "agent-1", 300)?;
/// ```
pub fn wait_for_slot(
    branch: &str,
    holder: &str,
    timeout: u64,
) -> Result<MergeSlot, MergeSlotError> {
    let start = Instant::now();
    let timeout_duration = Duration::from_secs(timeout);
    let poll_interval = Duration::from_secs(5);

    loop {
        // Check current status
        match check_slot(branch) {
            Ok(slot) => {
                if slot.status == SlotStatus::Available {
                    // Try to acquire
                    match acquire_slot(branch, holder, SlotConfig::new()) {
                        Ok(acquired) => return Ok(acquired),
                        Err(MergeSlotError::SlotHeld(_)) => {
                            // Race condition - someone else got it, keep waiting
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            Err(MergeSlotError::NotFound) => return Err(MergeSlotError::NotFound),
            Err(_) => {
                // Transient error, keep trying
            }
        }

        // Check timeout
        if start.elapsed() >= timeout_duration {
            return Err(MergeSlotError::Timeout(timeout));
        }

        // Wait before next poll
        thread::sleep(poll_interval);
    }
}

/// Force release a slot (admin operation)
///
/// Releases the slot regardless of who the current holder is.
/// Use with caution - this can interrupt an agent's merge operation.
///
/// # Arguments
/// * `branch` - Branch context
///
/// # Returns
/// Ok(()) on success
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::merge_slot::force_release;
///
/// // Admin: Force release a stuck slot
/// force_release("main")?;
/// ```
pub fn force_release(_branch: &str) -> Result<(), MergeSlotError> {
    // Force release by not specifying holder
    let args = vec!["merge-slot", "release"];

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("not found") || stderr.contains("no merge slot") {
            return Err(MergeSlotError::NotFound);
        }

        return Err(MergeSlotError::CliError(stderr.to_string()));
    }

    Ok(())
}

/// List all merge slots in the rig
///
/// Returns all merge slots (currently each rig has at most one).
///
/// # Returns
/// Vector of merge slots
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::merge_slot::list_slots;
///
/// let slots = list_slots()?;
/// for slot in slots {
///     println!("{}: {}", slot.slot_id, slot.status);
/// }
/// ```
pub fn list_slots() -> Result<Vec<MergeSlot>, MergeSlotError> {
    // Use bd merge-slot check --json to get current slot
    // bd doesn't have a list command, so we check if slot exists
    let args = vec!["merge-slot", "check", "--json"];

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("not found") || stderr.contains("no merge slot") {
            return Ok(Vec::new());
        }

        return Err(MergeSlotError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let slot = parse_slot_from_output(&stdout)?;

    Ok(vec![slot])
}

/// Create a merge slot for the current rig
///
/// Creates the merge slot bead if it doesn't exist.
///
/// # Returns
/// The created merge slot
pub fn create_slot() -> Result<MergeSlot, MergeSlotError> {
    let args = vec!["merge-slot", "create", "--json"];

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("already exists") {
            // Return existing slot
            return check_slot("");
        }

        return Err(MergeSlotError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_slot_from_output(&stdout)
}

/// Parse merge slot from bd command output
fn parse_slot_from_output(output: &str) -> Result<MergeSlot, MergeSlotError> {
    let trimmed = output.trim();

    // Handle empty output
    if trimmed.is_empty() {
        return Ok(MergeSlot::default());
    }

    // Try to parse as JSON
    if trimmed.starts_with('{') {
        let json: serde_json::Value = serde_json::from_str(trimmed)?;
        return parse_slot_from_json(&json);
    }

    // Handle text output (e.g., "available" or "held by agent-1")
    if trimmed.contains("available") {
        return Ok(MergeSlot {
            status: SlotStatus::Available,
            ..Default::default()
        });
    }

    if trimmed.contains("held") {
        let holder = extract_holder_from_message(trimmed);
        return Ok(MergeSlot {
            status: SlotStatus::Held,
            holder,
            ..Default::default()
        });
    }

    // Default to available if we can't parse
    Ok(MergeSlot::default())
}

/// Parse merge slot from JSON value
fn parse_slot_from_json(json: &serde_json::Value) -> Result<MergeSlot, MergeSlotError> {
    let slot_id = json
        .get("id")
        .or_else(|| json.get("slot_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let holder = json
        .get("holder")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let acquired_at = json
        .get("acquired_at")
        .or_else(|| json.get("acquiredAt"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let expires_at = json
        .get("expires_at")
        .or_else(|| json.get("expiresAt"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Parse status
    let status_str = json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("available");

    let status = status_str.parse().unwrap_or(SlotStatus::Available);

    let waiters = json
        .get("waiters")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    Ok(MergeSlot {
        slot_id,
        holder,
        acquired_at,
        expires_at,
        status,
        waiters,
    })
}

/// Extract holder from error message like "slot is held by agent-1"
fn extract_holder_from_error(message: &str) -> Option<String> {
    extract_holder_from_message(message)
}

/// Extract holder from message like "held by agent-1"
fn extract_holder_from_message(message: &str) -> Option<String> {
    // Look for patterns like "by <holder>" or "holder: <holder>"
    if let Some(pos) = message.find("by ") {
        let after_by = &message[pos + 3..];
        let holder = after_by
            .split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        if holder.is_some() {
            return holder;
        }
    }

    if let Some(pos) = message.find("holder:") {
        let after_holder = &message[pos + 7..];
        let holder = after_holder
            .trim()
            .split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        if holder.is_some() {
            return holder;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_status_from_str() {
        assert_eq!(
            SlotStatus::from_str("available").unwrap(),
            SlotStatus::Available
        );
        assert_eq!(SlotStatus::from_str("open").unwrap(), SlotStatus::Available);
        assert_eq!(SlotStatus::from_str("held").unwrap(), SlotStatus::Held);
        assert_eq!(
            SlotStatus::from_str("in_progress").unwrap(),
            SlotStatus::Held
        );
        assert_eq!(
            SlotStatus::from_str("expired").unwrap(),
            SlotStatus::Expired
        );
        assert_eq!(
            SlotStatus::from_str("timeout").unwrap(),
            SlotStatus::Expired
        );
    }

    #[test]
    fn test_slot_status_from_str_invalid() {
        assert!(SlotStatus::from_str("invalid").is_err());
        assert!(SlotStatus::from_str("").is_err());
    }

    #[test]
    fn test_slot_status_display() {
        assert_eq!(SlotStatus::Available.to_string(), "available");
        assert_eq!(SlotStatus::Held.to_string(), "held");
        assert_eq!(SlotStatus::Expired.to_string(), "expired");
    }

    #[test]
    fn test_slot_config_builder() {
        let config = SlotConfig::new()
            .with_timeout(300)
            .with_branch("main")
            .with_force(true)
            .with_wait(true);

        assert_eq!(config.timeout_seconds, 300);
        assert_eq!(config.branch, "main");
        assert!(config.force);
        assert!(config.wait_flag);
    }

    #[test]
    fn test_slot_config_default() {
        let config = SlotConfig::default();
        assert_eq!(config.timeout_seconds, 0);
        assert_eq!(config.branch, "");
        assert!(!config.force);
        assert!(!config.wait_flag);
    }

    #[test]
    fn test_merge_slot_default() {
        let slot = MergeSlot::default();
        assert!(slot.slot_id.is_empty());
        assert!(slot.holder.is_none());
        assert!(slot.acquired_at.is_none());
        assert!(slot.expires_at.is_none());
        assert_eq!(slot.status, SlotStatus::Available);
        assert!(slot.waiters.is_empty());
    }

    #[test]
    fn test_merge_slot_serialization() {
        let slot = MergeSlot {
            slot_id: "rb-merge-slot".to_string(),
            holder: Some("agent-1".to_string()),
            acquired_at: Some("2024-01-15T10:00:00Z".to_string()),
            expires_at: None,
            status: SlotStatus::Held,
            waiters: vec!["agent-2".to_string()],
        };

        let json = serde_json::to_string(&slot).unwrap();
        assert!(json.contains("\"slot_id\":\"rb-merge-slot\""));
        assert!(json.contains("\"holder\":\"agent-1\""));
        assert!(json.contains("\"status\":\"held\""));
        assert!(json.contains("\"waiters\":[\"agent-2\"]"));
    }

    #[test]
    fn test_merge_slot_deserialization() {
        let json = r#"{
            "slot_id": "test-slot",
            "holder": "agent-1",
            "status": "held",
            "waiters": ["agent-2", "agent-3"]
        }"#;

        let slot: MergeSlot = serde_json::from_str(json).unwrap();
        assert_eq!(slot.slot_id, "test-slot");
        assert_eq!(slot.holder, Some("agent-1".to_string()));
        assert_eq!(slot.status, SlotStatus::Held);
        assert_eq!(slot.waiters, vec!["agent-2", "agent-3"]);
    }

    #[test]
    fn test_slot_status_serialization() {
        let status = SlotStatus::Held;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"held\"");

        let deserialized: SlotStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SlotStatus::Held);
    }

    #[test]
    fn test_slot_status_default() {
        assert_eq!(SlotStatus::default(), SlotStatus::Available);
    }

    #[test]
    fn test_parse_slot_from_json_minimal() {
        let json = serde_json::json!({
            "status": "open"
        });

        let slot = parse_slot_from_json(&json).unwrap();
        assert_eq!(slot.status, SlotStatus::Available);
        assert!(slot.holder.is_none());
    }

    #[test]
    fn test_parse_slot_from_json_full() {
        let json = serde_json::json!({
            "id": "merge-slot",
            "holder": "agent-1",
            "status": "in_progress",
            "acquired_at": "2024-01-15T10:00:00Z",
            "waiters": ["agent-2"]
        });

        let slot = parse_slot_from_json(&json).unwrap();
        assert_eq!(slot.slot_id, "merge-slot");
        assert_eq!(slot.holder, Some("agent-1".to_string()));
        assert_eq!(slot.status, SlotStatus::Held);
        assert_eq!(slot.waiters, vec!["agent-2"]);
    }

    #[test]
    fn test_extract_holder_from_message() {
        assert_eq!(
            extract_holder_from_message("held by agent-1"),
            Some("agent-1".to_string())
        );
        assert_eq!(
            extract_holder_from_message("slot is held by worker-42"),
            Some("worker-42".to_string())
        );
        assert_eq!(
            extract_holder_from_message("holder: test-holder"),
            Some("test-holder".to_string())
        );
        assert_eq!(extract_holder_from_message("no holder info"), None);
    }

    #[test]
    fn test_parse_slot_from_output_text() {
        let slot = parse_slot_from_output("available").unwrap();
        assert_eq!(slot.status, SlotStatus::Available);

        let slot = parse_slot_from_output("slot is available").unwrap();
        assert_eq!(slot.status, SlotStatus::Available);

        let slot = parse_slot_from_output("held by agent-1").unwrap();
        assert_eq!(slot.status, SlotStatus::Held);
        assert_eq!(slot.holder, Some("agent-1".to_string()));
    }

    #[test]
    fn test_parse_slot_from_output_json() {
        let json_output = r#"{"status": "open", "holder": null}"#;
        let slot = parse_slot_from_output(json_output).unwrap();
        assert_eq!(slot.status, SlotStatus::Available);
    }

    #[test]
    fn test_parse_slot_from_output_empty() {
        let slot = parse_slot_from_output("").unwrap();
        assert_eq!(slot.status, SlotStatus::Available);
    }

    #[test]
    fn test_error_display() {
        let err = MergeSlotError::NotFound;
        assert_eq!(err.to_string(), "Merge slot not found for this rig");

        let err = MergeSlotError::SlotHeld("agent-1".to_string());
        assert_eq!(err.to_string(), "Slot is currently held by: agent-1");

        let err = MergeSlotError::Timeout(300);
        assert_eq!(
            err.to_string(),
            "Slot acquisition timed out after 300 seconds"
        );

        let err = MergeSlotError::NotHolder;
        assert_eq!(
            err.to_string(),
            "Not the current holder - cannot release slot"
        );
    }

    #[test]
    fn test_slot_status_case_insensitive() {
        assert_eq!(
            SlotStatus::from_str("AVAILABLE").unwrap(),
            SlotStatus::Available
        );
        assert_eq!(SlotStatus::from_str("HELD").unwrap(), SlotStatus::Held);
        assert_eq!(
            SlotStatus::from_str("In_Progress").unwrap(),
            SlotStatus::Held
        );
    }
}
