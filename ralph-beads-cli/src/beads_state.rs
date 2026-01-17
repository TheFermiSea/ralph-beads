//! Beads State Integration Module
//!
//! Wraps the beads CLI `bd set-state` and `bd state` commands to provide
//! type-safe state dimension management for ralph-beads workflows.
//!
//! State dimensions follow the convention `<dimension>:<value>` as labels:
//! - mode: planning, building, paused, complete
//! - health: healthy, degraded, failing

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::process::Command;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur during beads state operations
#[derive(Error, Debug)]
pub enum BeadsStateError {
    #[error("Invalid mode value: {0}. Valid values: planning, building, paused, complete")]
    InvalidMode(String),

    #[error("Invalid health value: {0}. Valid values: healthy, degraded, failing")]
    InvalidHealth(String),

    #[error("Invalid dimension: {0}. Valid dimensions: mode, health")]
    InvalidDimension(String),

    #[error("Beads CLI error: {0}")]
    CliError(String),

    #[error("Failed to execute bd command: {0}")]
    ExecutionError(#[from] std::io::Error),

    #[error("Failed to parse JSON output: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Workflow mode dimension values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Planning mode: creating proto with sequenced tasks
    Planning,
    /// Building mode: executing molecule until complete
    #[default]
    Building,
    /// Paused: workflow stopped, can be resumed
    Paused,
    /// Complete: workflow finished successfully
    Complete,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Planning => write!(f, "planning"),
            Mode::Building => write!(f, "building"),
            Mode::Paused => write!(f, "paused"),
            Mode::Complete => write!(f, "complete"),
        }
    }
}

impl FromStr for Mode {
    type Err = BeadsStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "planning" | "plan" => Ok(Mode::Planning),
            "building" | "build" => Ok(Mode::Building),
            "paused" | "pause" => Ok(Mode::Paused),
            "complete" | "done" => Ok(Mode::Complete),
            _ => Err(BeadsStateError::InvalidMode(s.to_string())),
        }
    }
}

/// Health dimension values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    /// All systems operational
    #[default]
    Healthy,
    /// Some issues but can continue
    Degraded,
    /// Critical issues, should stop
    Failing,
}

impl fmt::Display for Health {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Health::Healthy => write!(f, "healthy"),
            Health::Degraded => write!(f, "degraded"),
            Health::Failing => write!(f, "failing"),
        }
    }
}

impl FromStr for Health {
    type Err = BeadsStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "healthy" | "ok" => Ok(Health::Healthy),
            "degraded" | "warning" => Ok(Health::Degraded),
            "failing" | "failed" | "error" => Ok(Health::Failing),
            _ => Err(BeadsStateError::InvalidHealth(s.to_string())),
        }
    }
}

/// Supported state dimensions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dimension {
    Mode,
    Health,
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dimension::Mode => write!(f, "mode"),
            Dimension::Health => write!(f, "health"),
        }
    }
}

impl FromStr for Dimension {
    type Err = BeadsStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mode" => Ok(Dimension::Mode),
            "health" => Ok(Dimension::Health),
            _ => Err(BeadsStateError::InvalidDimension(s.to_string())),
        }
    }
}

/// Result of a state operation
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// The issue ID
    pub issue_id: String,
    /// The dimension that was operated on
    pub dimension: String,
    /// The value (for get operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Error message (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Set the mode dimension for an issue
///
/// # Arguments
/// * `issue_id` - The beads issue ID
/// * `mode` - The mode value to set
/// * `reason` - Optional reason for the state change
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::beads_state::{set_mode, Mode};
/// set_mode("ralph-beads-123", Mode::Building, Some("Starting build phase"))?;
/// ```
#[allow(dead_code)]
pub fn set_mode(issue_id: &str, mode: Mode, reason: Option<&str>) -> Result<(), BeadsStateError> {
    set_state_dimension(issue_id, "mode", &mode.to_string(), reason)
}

/// Get the current mode for an issue
///
/// Returns `None` if the mode dimension is not set.
#[allow(dead_code)]
pub fn get_mode(issue_id: &str) -> Result<Option<Mode>, BeadsStateError> {
    match get_state_dimension(issue_id, "mode")? {
        Some(value) => Ok(Some(value.parse()?)),
        None => Ok(None),
    }
}

/// Set the health dimension for an issue
///
/// # Arguments
/// * `issue_id` - The beads issue ID
/// * `health` - The health value to set
/// * `reason` - Optional reason for the state change
#[allow(dead_code)]
pub fn set_health(
    issue_id: &str,
    health: Health,
    reason: Option<&str>,
) -> Result<(), BeadsStateError> {
    set_state_dimension(issue_id, "health", &health.to_string(), reason)
}

/// Get the current health for an issue
///
/// Returns `None` if the health dimension is not set.
#[allow(dead_code)]
pub fn get_health(issue_id: &str) -> Result<Option<Health>, BeadsStateError> {
    match get_state_dimension(issue_id, "health")? {
        Some(value) => Ok(Some(value.parse()?)),
        None => Ok(None),
    }
}

/// Get all state dimensions for an issue
///
/// Returns a HashMap of dimension name to value.
pub fn get_all_state(issue_id: &str) -> Result<HashMap<String, String>, BeadsStateError> {
    let output = Command::new("bd")
        .args(["state", "list", issue_id, "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check for "no state labels" which is a valid empty state
        if stderr.contains("no state") || stderr.contains("not found") {
            return Ok(HashMap::new());
        }
        return Err(BeadsStateError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Try to parse as JSON array of {dimension, value} objects
    if let Ok(states) = serde_json::from_str::<Vec<StateEntry>>(&stdout) {
        let mut result = HashMap::new();
        for entry in states {
            result.insert(entry.dimension, entry.value);
        }
        return Ok(result);
    }

    // Try to parse as HashMap directly
    if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&stdout) {
        return Ok(map);
    }

    // If parsing fails but command succeeded, return empty map
    Ok(HashMap::new())
}

/// Internal helper struct for parsing state list output
#[derive(Debug, Deserialize)]
struct StateEntry {
    dimension: String,
    value: String,
}

/// Set a generic state dimension (low-level function)
///
/// This is the internal implementation that calls `bd set-state`.
fn set_state_dimension(
    issue_id: &str,
    dimension: &str,
    value: &str,
    reason: Option<&str>,
) -> Result<(), BeadsStateError> {
    let assignment = format!("{}={}", dimension, value);

    let mut args = vec!["set-state", issue_id, &assignment];

    let reason_string;
    if let Some(r) = reason {
        reason_string = r.to_string();
        args.push("--reason");
        args.push(&reason_string);
    }

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BeadsStateError::CliError(stderr.to_string()));
    }

    Ok(())
}

/// Get a generic state dimension (low-level function)
///
/// Returns `None` if the dimension is not set.
fn get_state_dimension(issue_id: &str, dimension: &str) -> Result<Option<String>, BeadsStateError> {
    let output = Command::new("bd")
        .args(["state", issue_id, dimension])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check for "not set" or "not found" which means no value
        if stderr.contains("not set") || stderr.contains("not found") || stderr.contains("no value")
        {
            return Ok(None);
        }
        return Err(BeadsStateError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout.is_empty() {
        return Ok(None);
    }

    Ok(Some(stdout))
}

/// Set any dimension with validation
///
/// This validates the dimension and value before calling the CLI.
pub fn set_state(
    issue_id: &str,
    dimension: &str,
    value: &str,
    reason: Option<&str>,
) -> Result<(), BeadsStateError> {
    // Validate dimension
    let dim: Dimension = dimension.parse()?;

    // Validate value based on dimension
    match dim {
        Dimension::Mode => {
            let _: Mode = value.parse()?;
        }
        Dimension::Health => {
            let _: Health = value.parse()?;
        }
    }

    set_state_dimension(issue_id, dimension, value, reason)
}

/// Get a specific dimension with validation
pub fn get_state(issue_id: &str, dimension: &str) -> Result<Option<String>, BeadsStateError> {
    // Validate dimension
    let _: Dimension = dimension.parse()?;

    get_state_dimension(issue_id, dimension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_from_str() {
        assert_eq!(Mode::from_str("planning").unwrap(), Mode::Planning);
        assert_eq!(Mode::from_str("plan").unwrap(), Mode::Planning);
        assert_eq!(Mode::from_str("PLANNING").unwrap(), Mode::Planning);
        assert_eq!(Mode::from_str("building").unwrap(), Mode::Building);
        assert_eq!(Mode::from_str("build").unwrap(), Mode::Building);
        assert_eq!(Mode::from_str("paused").unwrap(), Mode::Paused);
        assert_eq!(Mode::from_str("pause").unwrap(), Mode::Paused);
        assert_eq!(Mode::from_str("complete").unwrap(), Mode::Complete);
        assert_eq!(Mode::from_str("done").unwrap(), Mode::Complete);
    }

    #[test]
    fn test_mode_from_str_invalid() {
        assert!(Mode::from_str("invalid").is_err());
        assert!(Mode::from_str("").is_err());
    }

    #[test]
    fn test_mode_display() {
        assert_eq!(Mode::Planning.to_string(), "planning");
        assert_eq!(Mode::Building.to_string(), "building");
        assert_eq!(Mode::Paused.to_string(), "paused");
        assert_eq!(Mode::Complete.to_string(), "complete");
    }

    #[test]
    fn test_health_from_str() {
        assert_eq!(Health::from_str("healthy").unwrap(), Health::Healthy);
        assert_eq!(Health::from_str("ok").unwrap(), Health::Healthy);
        assert_eq!(Health::from_str("HEALTHY").unwrap(), Health::Healthy);
        assert_eq!(Health::from_str("degraded").unwrap(), Health::Degraded);
        assert_eq!(Health::from_str("warning").unwrap(), Health::Degraded);
        assert_eq!(Health::from_str("failing").unwrap(), Health::Failing);
        assert_eq!(Health::from_str("failed").unwrap(), Health::Failing);
        assert_eq!(Health::from_str("error").unwrap(), Health::Failing);
    }

    #[test]
    fn test_health_from_str_invalid() {
        assert!(Health::from_str("invalid").is_err());
        assert!(Health::from_str("").is_err());
    }

    #[test]
    fn test_health_display() {
        assert_eq!(Health::Healthy.to_string(), "healthy");
        assert_eq!(Health::Degraded.to_string(), "degraded");
        assert_eq!(Health::Failing.to_string(), "failing");
    }

    #[test]
    fn test_dimension_from_str() {
        assert_eq!(Dimension::from_str("mode").unwrap(), Dimension::Mode);
        assert_eq!(Dimension::from_str("MODE").unwrap(), Dimension::Mode);
        assert_eq!(Dimension::from_str("health").unwrap(), Dimension::Health);
        assert_eq!(Dimension::from_str("HEALTH").unwrap(), Dimension::Health);
    }

    #[test]
    fn test_dimension_from_str_invalid() {
        assert!(Dimension::from_str("invalid").is_err());
        assert!(Dimension::from_str("patrol").is_err()); // valid bd dimension but not supported here
    }

    #[test]
    fn test_dimension_display() {
        assert_eq!(Dimension::Mode.to_string(), "mode");
        assert_eq!(Dimension::Health.to_string(), "health");
    }

    #[test]
    fn test_mode_serialization() {
        let mode = Mode::Planning;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"planning\"");

        let deserialized: Mode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Mode::Planning);
    }

    #[test]
    fn test_health_serialization() {
        let health = Health::Degraded;
        let json = serde_json::to_string(&health).unwrap();
        assert_eq!(json, "\"degraded\"");

        let deserialized: Health = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Health::Degraded);
    }

    #[test]
    fn test_set_state_validates_dimension() {
        // This should fail validation before calling CLI
        let result = set_state("test-id", "invalid_dimension", "value", None);
        assert!(result.is_err());

        match result {
            Err(BeadsStateError::InvalidDimension(dim)) => {
                assert_eq!(dim, "invalid_dimension");
            }
            _ => panic!("Expected InvalidDimension error"),
        }
    }

    #[test]
    fn test_set_state_validates_mode_value() {
        // Valid dimension but invalid value
        let result = set_state("test-id", "mode", "invalid_mode", None);
        assert!(result.is_err());

        match result {
            Err(BeadsStateError::InvalidMode(mode)) => {
                assert_eq!(mode, "invalid_mode");
            }
            _ => panic!("Expected InvalidMode error"),
        }
    }

    #[test]
    fn test_set_state_validates_health_value() {
        // Valid dimension but invalid value
        let result = set_state("test-id", "health", "invalid_health", None);
        assert!(result.is_err());

        match result {
            Err(BeadsStateError::InvalidHealth(health)) => {
                assert_eq!(health, "invalid_health");
            }
            _ => panic!("Expected InvalidHealth error"),
        }
    }

    #[test]
    fn test_get_state_validates_dimension() {
        let result = get_state("test-id", "invalid_dimension");
        assert!(result.is_err());

        match result {
            Err(BeadsStateError::InvalidDimension(dim)) => {
                assert_eq!(dim, "invalid_dimension");
            }
            _ => panic!("Expected InvalidDimension error"),
        }
    }

    #[test]
    fn test_state_result_serialization() {
        let result = StateResult {
            success: true,
            issue_id: "test-123".to_string(),
            dimension: "mode".to_string(),
            value: Some("building".to_string()),
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"issue_id\":\"test-123\""));
        assert!(json.contains("\"dimension\":\"mode\""));
        assert!(json.contains("\"value\":\"building\""));
        // error should be skipped when None
        assert!(!json.contains("error"));
    }

    #[test]
    fn test_default_values() {
        assert_eq!(Mode::default(), Mode::Building);
        assert_eq!(Health::default(), Health::Healthy);
    }
}
