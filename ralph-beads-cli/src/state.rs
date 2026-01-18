//! Workflow state types for Ralph-Beads
//!
//! Provides the WorkflowMode enum used by iteration calculation.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Workflow modes for Ralph-Beads execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowMode {
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

impl fmt::Display for WorkflowMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkflowMode::Planning => write!(f, "planning"),
            WorkflowMode::Building => write!(f, "building"),
            WorkflowMode::Paused => write!(f, "paused"),
            WorkflowMode::Complete => write!(f, "complete"),
        }
    }
}

impl FromStr for WorkflowMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "planning" | "plan" => Ok(WorkflowMode::Planning),
            "building" | "build" => Ok(WorkflowMode::Building),
            "paused" | "pause" => Ok(WorkflowMode::Paused),
            "complete" | "done" => Ok(WorkflowMode::Complete),
            _ => Err(format!("Unknown workflow mode: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_mode_from_str() {
        assert_eq!(
            "planning".parse::<WorkflowMode>().unwrap(),
            WorkflowMode::Planning
        );
        assert_eq!(
            "plan".parse::<WorkflowMode>().unwrap(),
            WorkflowMode::Planning
        );
        assert_eq!(
            "building".parse::<WorkflowMode>().unwrap(),
            WorkflowMode::Building
        );
        assert_eq!(
            "build".parse::<WorkflowMode>().unwrap(),
            WorkflowMode::Building
        );
        assert_eq!(
            "paused".parse::<WorkflowMode>().unwrap(),
            WorkflowMode::Paused
        );
        assert_eq!(
            "pause".parse::<WorkflowMode>().unwrap(),
            WorkflowMode::Paused
        );
        assert_eq!(
            "complete".parse::<WorkflowMode>().unwrap(),
            WorkflowMode::Complete
        );
        assert_eq!(
            "done".parse::<WorkflowMode>().unwrap(),
            WorkflowMode::Complete
        );
    }

    #[test]
    fn test_workflow_mode_display() {
        assert_eq!(WorkflowMode::Planning.to_string(), "planning");
        assert_eq!(WorkflowMode::Building.to_string(), "building");
        assert_eq!(WorkflowMode::Paused.to_string(), "paused");
        assert_eq!(WorkflowMode::Complete.to_string(), "complete");
    }

    #[test]
    fn test_workflow_mode_default() {
        assert_eq!(WorkflowMode::default(), WorkflowMode::Building);
    }

    #[test]
    fn test_workflow_mode_serialization() {
        let mode = WorkflowMode::Planning;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"planning\"");

        let deserialized: WorkflowMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, WorkflowMode::Planning);
    }
}
