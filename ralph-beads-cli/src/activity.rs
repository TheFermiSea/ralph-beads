//! Activity Feed Integration
//!
//! Provides real-time progress monitoring via the `bd activity` command.
//! This module wraps beads CLI activity commands and parses their output
//! into structured Rust types.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use thiserror::Error;

/// Types of activity events from beads
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    /// Issue created
    Create,
    /// Issue updated (general update)
    Update,
    /// Issue deleted
    Delete,
    /// Status change (open -> in_progress, etc.)
    Status,
    /// Comment added
    Comment,
    /// Unknown event type
    Unknown,
}

impl Default for ActivityType {
    fn default() -> Self {
        ActivityType::Unknown
    }
}

impl fmt::Display for ActivityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActivityType::Create => write!(f, "create"),
            ActivityType::Update => write!(f, "update"),
            ActivityType::Delete => write!(f, "delete"),
            ActivityType::Status => write!(f, "status"),
            ActivityType::Comment => write!(f, "comment"),
            ActivityType::Unknown => write!(f, "unknown"),
        }
    }
}

impl FromStr for ActivityType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "create" | "created" => Ok(ActivityType::Create),
            "update" | "updated" => Ok(ActivityType::Update),
            "delete" | "deleted" => Ok(ActivityType::Delete),
            "status" => Ok(ActivityType::Status),
            "comment" => Ok(ActivityType::Comment),
            _ => Ok(ActivityType::Unknown),
        }
    }
}

/// A single activity event from the beads activity feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    /// Timestamp of the event (ISO 8601 format)
    pub timestamp: String,

    /// Issue ID this event relates to
    pub issue_id: String,

    /// Type of event
    pub event_type: ActivityType,

    /// Human-readable summary/message
    pub summary: String,

    /// Actor who triggered the event (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,

    /// Symbol used in display (e.g., +, ->, checkmark)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,

    /// Old status (for status change events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_status: Option<String>,

    /// New status (for status change events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_status: Option<String>,
}

/// Raw JSON structure from `bd activity --json`
#[derive(Debug, Clone, Deserialize)]
struct RawActivityEvent {
    timestamp: String,
    #[serde(rename = "type")]
    event_type: Option<String>,
    issue_id: Option<String>,
    symbol: Option<String>,
    message: Option<String>,
    actor: Option<String>,
    old_status: Option<String>,
    new_status: Option<String>,
}

impl From<RawActivityEvent> for ActivityEvent {
    fn from(raw: RawActivityEvent) -> Self {
        let event_type = raw
            .event_type
            .as_deref()
            .map(|s| s.parse().unwrap_or(ActivityType::Unknown))
            .unwrap_or(ActivityType::Unknown);

        ActivityEvent {
            timestamp: raw.timestamp,
            issue_id: raw.issue_id.unwrap_or_default(),
            event_type,
            summary: raw.message.unwrap_or_default(),
            actor: raw.actor,
            symbol: raw.symbol,
            old_status: raw.old_status,
            new_status: raw.new_status,
        }
    }
}

/// Errors that can occur during activity operations
#[derive(Error, Debug)]
pub enum ActivityError {
    #[error("Failed to execute bd command: {0}")]
    CommandFailed(String),

    #[error("Failed to parse activity output: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Activity stream for real-time monitoring (--follow mode)
pub struct ActivityStream {
    /// The child process running `bd activity --follow`
    child: Child,
    /// Buffered reader for stdout
    reader: BufReader<std::process::ChildStdout>,
}

impl ActivityStream {
    /// Create a new activity stream
    fn new(child: Child, reader: BufReader<std::process::ChildStdout>) -> Self {
        Self { child, reader }
    }

    /// Read the next activity event from the stream
    ///
    /// Returns `None` if the stream has ended or an error occurred.
    pub fn next_event(&mut self) -> Option<Result<ActivityEvent, ActivityError>> {
        let mut line = String::new();

        match self.reader.read_line(&mut line) {
            Ok(0) => None, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    // Skip empty lines, try next
                    return self.next_event();
                }

                // Parse as JSON (single event)
                match serde_json::from_str::<RawActivityEvent>(trimmed) {
                    Ok(raw) => Some(Ok(raw.into())),
                    Err(e) => {
                        // Try parsing as part of a JSON array (shouldn't happen in follow mode)
                        Some(Err(ActivityError::ParseError(format!(
                            "Failed to parse event: {} - line: {}",
                            e, trimmed
                        ))))
                    }
                }
            }
            Err(e) => Some(Err(ActivityError::IoError(e))),
        }
    }

    /// Stop the activity stream
    pub fn stop(mut self) -> Result<(), ActivityError> {
        self.child.kill().ok(); // Ignore error if already dead
        self.child.wait()?;
        Ok(())
    }
}

impl Iterator for ActivityStream {
    type Item = Result<ActivityEvent, ActivityError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_event()
    }
}

impl Drop for ActivityStream {
    fn drop(&mut self) {
        // Try to kill the child process if it's still running
        self.child.kill().ok();
    }
}

/// Get recent activity events
///
/// # Arguments
/// * `limit` - Maximum number of events to return
///
/// # Returns
/// A vector of activity events, most recent first
pub fn get_recent_activity(limit: usize) -> Result<Vec<ActivityEvent>, ActivityError> {
    let output = Command::new("bd")
        .args(["activity", "--json", "--limit", &limit.to_string()])
        .output()
        .map_err(|e| ActivityError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ActivityError::CommandFailed(format!(
            "bd activity failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_activity_json(&stdout)
}

/// Get activity events for a specific issue
///
/// # Arguments
/// * `issue_id` - The issue ID or prefix to filter by
/// * `limit` - Maximum number of events to return
///
/// # Returns
/// A vector of activity events for the specified issue
pub fn get_activity_for_issue(
    issue_id: &str,
    limit: usize,
) -> Result<Vec<ActivityEvent>, ActivityError> {
    let output = Command::new("bd")
        .args([
            "activity",
            "--json",
            "--mol",
            issue_id,
            "--limit",
            &limit.to_string(),
        ])
        .output()
        .map_err(|e| ActivityError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ActivityError::CommandFailed(format!(
            "bd activity failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_activity_json(&stdout)
}

/// Get activity events since a duration
///
/// # Arguments
/// * `since` - Duration string (e.g., "5m", "1h", "30s")
/// * `issue_id` - Optional issue ID to filter by
///
/// # Returns
/// A vector of activity events since the specified duration
pub fn get_activity_since(
    since: &str,
    issue_id: Option<&str>,
) -> Result<Vec<ActivityEvent>, ActivityError> {
    let mut args = vec!["activity", "--json", "--since", since];

    if let Some(id) = issue_id {
        args.push("--mol");
        args.push(id);
    }

    let output = Command::new("bd")
        .args(&args)
        .output()
        .map_err(|e| ActivityError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ActivityError::CommandFailed(format!(
            "bd activity failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_activity_json(&stdout)
}

/// Start streaming activity events (real-time) and print to stdout
///
/// # Arguments
/// * `issue_id` - Optional issue ID to filter by
/// * `limit` - Maximum number of events to receive (0 = unlimited)
/// * `format` - Output format: "json" or "text"
///
/// # Returns
/// Result indicating success or failure
pub fn stream_activity(
    issue_id: Option<&str>,
    limit: usize,
    format: &str,
) -> Result<(), ActivityError> {
    let mut cmd = Command::new("bd");
    cmd.args(["activity", "--follow", "--json"]);

    if let Some(id) = issue_id {
        cmd.args(["--mol", id]);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null()); // Suppress stderr

    let mut child = cmd
        .spawn()
        .map_err(|e| ActivityError::CommandFailed(e.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ActivityError::CommandFailed("Failed to capture stdout".to_string()))?;

    let reader = BufReader::new(stdout);
    let mut count = 0;

    for line_result in reader.lines() {
        match line_result {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match serde_json::from_str::<RawActivityEvent>(trimmed) {
                    Ok(raw) => {
                        let event: ActivityEvent = raw.into();
                        if format == "json" {
                            println!("{}", serde_json::to_string(&event).unwrap());
                        } else {
                            print!("{}", format_activity_text(&[event]));
                        }

                        count += 1;
                        if limit > 0 && count >= limit {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to parse event: {}", e);
                    }
                }
            }
            Err(e) => {
                return Err(ActivityError::IoError(e));
            }
        }
    }

    // Clean up child process
    child.kill().ok();
    child.wait()?;

    Ok(())
}

/// Create an activity stream for iteration
///
/// # Arguments
/// * `issue_id` - Optional issue ID to filter by
///
/// # Returns
/// An ActivityStream that yields events as they occur
pub fn create_activity_stream(issue_id: Option<&str>) -> Result<ActivityStream, ActivityError> {
    let mut cmd = Command::new("bd");
    cmd.args(["activity", "--follow", "--json"]);

    if let Some(id) = issue_id {
        cmd.args(["--mol", id]);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null()); // Suppress stderr

    let mut child = cmd
        .spawn()
        .map_err(|e| ActivityError::CommandFailed(e.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ActivityError::CommandFailed("Failed to capture stdout".to_string()))?;

    let reader = BufReader::new(stdout);

    Ok(ActivityStream::new(child, reader))
}

/// Parse JSON output from `bd activity --json`
fn parse_activity_json(json_str: &str) -> Result<Vec<ActivityEvent>, ActivityError> {
    let trimmed = json_str.trim();

    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // bd activity --json returns a JSON array
    let raw_events: Vec<RawActivityEvent> = serde_json::from_str(trimmed)?;

    Ok(raw_events.into_iter().map(|r| r.into()).collect())
}

/// Format activity events for human-readable output
pub fn format_activity_text(events: &[ActivityEvent]) -> String {
    let mut output = String::new();

    for event in events {
        let symbol = event.symbol.as_deref().unwrap_or(" ");
        let actor_suffix = event
            .actor
            .as_ref()
            .map(|a| format!(" @{}", a))
            .unwrap_or_default();

        // Extract time portion from timestamp
        let time = extract_time(&event.timestamp);

        output.push_str(&format!(
            "[{}] {} {} · {}{}\n",
            time, symbol, event.issue_id, event.summary, actor_suffix
        ));
    }

    output
}

/// Extract time portion from ISO 8601 timestamp
fn extract_time(timestamp: &str) -> &str {
    // Format: 2026-01-17T09:31:44.215236-06:00
    // We want: 09:31:44
    if let Some(t_pos) = timestamp.find('T') {
        let after_t = &timestamp[t_pos + 1..];
        if let Some(dot_pos) = after_t.find('.') {
            return &after_t[..dot_pos];
        }
        // No decimal, look for timezone
        if after_t.len() >= 8 {
            return &after_t[..8];
        }
    }
    timestamp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_type_from_str() {
        assert_eq!(
            "create".parse::<ActivityType>().unwrap(),
            ActivityType::Create
        );
        assert_eq!(
            "created".parse::<ActivityType>().unwrap(),
            ActivityType::Create
        );
        assert_eq!(
            "update".parse::<ActivityType>().unwrap(),
            ActivityType::Update
        );
        assert_eq!(
            "status".parse::<ActivityType>().unwrap(),
            ActivityType::Status
        );
        assert_eq!(
            "comment".parse::<ActivityType>().unwrap(),
            ActivityType::Comment
        );
        assert_eq!(
            "delete".parse::<ActivityType>().unwrap(),
            ActivityType::Delete
        );
        assert_eq!(
            "unknown_type".parse::<ActivityType>().unwrap(),
            ActivityType::Unknown
        );
    }

    #[test]
    fn test_activity_type_display() {
        assert_eq!(ActivityType::Create.to_string(), "create");
        assert_eq!(ActivityType::Update.to_string(), "update");
        assert_eq!(ActivityType::Status.to_string(), "status");
        assert_eq!(ActivityType::Comment.to_string(), "comment");
        assert_eq!(ActivityType::Delete.to_string(), "delete");
        assert_eq!(ActivityType::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_parse_activity_json() {
        let json = r#"[
            {
                "timestamp": "2026-01-17T09:31:44.215236-06:00",
                "type": "status",
                "issue_id": "ralph-beads-73t.9",
                "symbol": "✓",
                "message": "ralph-beads-73t.9 completed · Add file locking...",
                "old_status": "in_progress",
                "new_status": "closed"
            },
            {
                "timestamp": "2026-01-17T09:32:21.95706-06:00",
                "type": "update",
                "issue_id": "ralph-beads-73t",
                "symbol": "→",
                "message": "ralph-beads-73t updated · Advanced Beads Integration...",
                "actor": "TheFermiSea"
            }
        ]"#;

        let events = parse_activity_json(json).unwrap();
        assert_eq!(events.len(), 2);

        assert_eq!(events[0].issue_id, "ralph-beads-73t.9");
        assert_eq!(events[0].event_type, ActivityType::Status);
        assert_eq!(events[0].old_status, Some("in_progress".to_string()));
        assert_eq!(events[0].new_status, Some("closed".to_string()));

        assert_eq!(events[1].issue_id, "ralph-beads-73t");
        assert_eq!(events[1].event_type, ActivityType::Update);
        assert_eq!(events[1].actor, Some("TheFermiSea".to_string()));
    }

    #[test]
    fn test_parse_empty_json() {
        let events = parse_activity_json("").unwrap();
        assert!(events.is_empty());

        let events = parse_activity_json("[]").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_extract_time() {
        assert_eq!(extract_time("2026-01-17T09:31:44.215236-06:00"), "09:31:44");
        assert_eq!(extract_time("2026-01-17T13:36:24.933293-06:00"), "13:36:24");
        assert_eq!(extract_time("invalid"), "invalid");
    }

    #[test]
    fn test_format_activity_text() {
        let events = vec![
            ActivityEvent {
                timestamp: "2026-01-17T09:31:44.215236-06:00".to_string(),
                issue_id: "ralph-beads-73t.9".to_string(),
                event_type: ActivityType::Status,
                summary: "completed task".to_string(),
                actor: None,
                symbol: Some("✓".to_string()),
                old_status: Some("in_progress".to_string()),
                new_status: Some("closed".to_string()),
            },
            ActivityEvent {
                timestamp: "2026-01-17T09:32:21.95706-06:00".to_string(),
                issue_id: "ralph-beads-73t".to_string(),
                event_type: ActivityType::Update,
                summary: "updated epic".to_string(),
                actor: Some("User1".to_string()),
                symbol: Some("→".to_string()),
                old_status: None,
                new_status: None,
            },
        ];

        let text = format_activity_text(&events);
        assert!(text.contains("[09:31:44]"));
        assert!(text.contains("ralph-beads-73t.9"));
        assert!(text.contains("@User1"));
    }

    #[test]
    fn test_activity_event_serialization() {
        let event = ActivityEvent {
            timestamp: "2026-01-17T09:31:44.215236-06:00".to_string(),
            issue_id: "test-123".to_string(),
            event_type: ActivityType::Create,
            summary: "Test event".to_string(),
            actor: Some("user".to_string()),
            symbol: Some("+".to_string()),
            old_status: None,
            new_status: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ActivityEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.issue_id, event.issue_id);
        assert_eq!(deserialized.event_type, event.event_type);
        assert_eq!(deserialized.actor, event.actor);
    }

    #[test]
    fn test_raw_to_activity_event_conversion() {
        let raw = RawActivityEvent {
            timestamp: "2026-01-17T10:00:00-06:00".to_string(),
            event_type: Some("create".to_string()),
            issue_id: Some("issue-1".to_string()),
            symbol: Some("+".to_string()),
            message: Some("Created issue".to_string()),
            actor: Some("dev".to_string()),
            old_status: None,
            new_status: None,
        };

        let event: ActivityEvent = raw.into();

        assert_eq!(event.timestamp, "2026-01-17T10:00:00-06:00");
        assert_eq!(event.event_type, ActivityType::Create);
        assert_eq!(event.issue_id, "issue-1");
        assert_eq!(event.summary, "Created issue");
        assert_eq!(event.actor, Some("dev".to_string()));
    }
}
