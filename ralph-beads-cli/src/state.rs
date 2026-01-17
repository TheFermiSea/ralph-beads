use crate::complexity::Complexity;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Workflow modes for Ralph-Beads execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowMode {
    /// Planning mode: creating proto with sequenced tasks
    Planning,
    /// Building mode: executing molecule until complete
    Building,
    /// Paused: workflow stopped, can be resumed
    Paused,
    /// Complete: workflow finished successfully
    Complete,
}

impl Default for WorkflowMode {
    fn default() -> Self {
        WorkflowMode::Building
    }
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
            "paused" => Ok(WorkflowMode::Paused),
            "complete" | "done" => Ok(WorkflowMode::Complete),
            _ => Err(format!("Unknown workflow mode: {}", s)),
        }
    }
}

impl WorkflowMode {
    /// Get the completion promise for this mode
    pub fn completion_promise(&self) -> &'static str {
        match self {
            WorkflowMode::Planning => "PLAN_READY",
            WorkflowMode::Building => "DONE",
            _ => "",
        }
    }

    /// Check if this mode is active (can run iterations)
    pub fn is_active(&self) -> bool {
        matches!(self, WorkflowMode::Planning | WorkflowMode::Building)
    }
}

/// Promise types that can complete a workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PromiseType {
    /// Planning complete
    PlanReady,
    /// Building complete
    Done,
}

impl fmt::Display for PromiseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromiseType::PlanReady => write!(f, "PLAN_READY"),
            PromiseType::Done => write!(f, "DONE"),
        }
    }
}

impl FromStr for PromiseType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PLAN_READY" => Ok(PromiseType::PlanReady),
            "DONE" => Ok(PromiseType::Done),
            _ => Err(format!("Unknown promise type: {}", s)),
        }
    }
}

/// Errors that can occur during state operations
#[derive(Error, Debug)]
pub enum StateError {
    #[error("Unknown field: {0}")]
    UnknownField(String),

    #[error("Invalid value for {field}: {value}")]
    InvalidValue { field: String, value: String },

    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Session state for Ralph-Beads workflow
///
/// This struct mirrors the TypeScript SessionState but with Rust type safety.
/// It can be serialized to/from JSON for interop with the TypeScript plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Unique session identifier
    pub session_id: String,

    /// Epic ID being worked on
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_id: Option<String>,

    /// Molecule ID (for building mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub molecule_id: Option<String>,

    /// Current workflow mode
    pub mode: WorkflowMode,

    /// Task complexity level
    pub complexity: Complexity,

    /// Current task being worked on
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task: Option<String>,

    /// Number of iterations completed
    pub iteration_count: u32,

    /// Number of consecutive failures
    pub failure_count: u32,

    /// Maximum allowed iterations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,

    /// Promise that has been made (PLAN_READY or DONE)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promise_made: Option<PromiseType>,

    /// Worktree path (if using git worktree)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,

    /// Branch name (if using git worktree)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,

    /// Whether to create PR on completion
    #[serde(default)]
    pub create_pr: bool,

    /// Files modified during this session
    #[serde(default)]
    pub files_modified: Vec<String>,

    /// Whether a commit has been made
    #[serde(default)]
    pub commit_made: bool,

    /// Whether tests have been run
    #[serde(default)]
    pub tests_ran: bool,
}

impl SessionState {
    /// Create a new session state with the given session ID
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            epic_id: None,
            molecule_id: None,
            mode: WorkflowMode::default(),
            complexity: Complexity::default(),
            current_task: None,
            iteration_count: 0,
            failure_count: 0,
            max_iterations: None,
            promise_made: None,
            worktree_path: None,
            branch_name: None,
            create_pr: false,
            files_modified: Vec::new(),
            commit_made: false,
            tests_ran: false,
        }
    }

    /// Builder pattern: set mode
    pub fn with_mode(mut self, mode: WorkflowMode) -> Self {
        self.mode = mode;
        self
    }

    /// Builder pattern: set epic ID
    pub fn with_epic_id(mut self, epic_id: Option<String>) -> Self {
        self.epic_id = epic_id;
        self
    }

    /// Builder pattern: set molecule ID
    pub fn with_molecule_id(mut self, molecule_id: Option<String>) -> Self {
        self.molecule_id = molecule_id;
        self
    }

    /// Builder pattern: set complexity
    pub fn with_complexity(mut self, complexity: Complexity) -> Self {
        self.complexity = complexity;
        self
    }

    /// Builder pattern: set max iterations
    pub fn with_max_iterations(mut self, max_iterations: Option<u32>) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Increment iteration count
    pub fn increment_iteration(&mut self) {
        self.iteration_count += 1;
    }

    /// Increment failure count
    pub fn increment_failure(&mut self) {
        self.failure_count += 1;
    }

    /// Reset failure count (on success)
    pub fn reset_failures(&mut self) {
        self.failure_count = 0;
    }

    /// Record a promise being made
    pub fn set_promise(&mut self, promise: PromiseType) {
        self.promise_made = Some(promise);
    }

    /// Add a modified file
    pub fn add_modified_file(&mut self, path: String) {
        if !self.files_modified.contains(&path) {
            self.files_modified.push(path);
        }
    }

    /// Check if the loop should continue
    pub fn should_continue(&self) -> bool {
        // Check if mode is active
        if !self.mode.is_active() {
            return false;
        }

        // Check if max iterations reached
        if let Some(max) = self.max_iterations {
            if self.iteration_count >= max {
                return false;
            }
        }

        // Check if promise has been fulfilled
        if let Some(promise) = &self.promise_made {
            let expected = match self.mode {
                WorkflowMode::Planning => PromiseType::PlanReady,
                WorkflowMode::Building => PromiseType::Done,
                _ => return false,
            };
            if *promise == expected {
                return false;
            }
        }

        true
    }

    /// Get the reason for continuation decision
    pub fn continuation_reason(&self) -> &'static str {
        if !self.mode.is_active() {
            return "mode_inactive";
        }

        if let Some(max) = self.max_iterations {
            if self.iteration_count >= max {
                return "max_iterations_reached";
            }
        }

        if let Some(promise) = &self.promise_made {
            let expected = match self.mode {
                WorkflowMode::Planning => PromiseType::PlanReady,
                WorkflowMode::Building => PromiseType::Done,
                _ => return "mode_inactive",
            };
            if *promise == expected {
                return "promise_fulfilled";
            }
        }

        "work_remaining"
    }

    /// Update a field by name (for CLI interface)
    pub fn update_field(&mut self, field: &str, value: &str) -> Result<(), StateError> {
        match field {
            "mode" => {
                self.mode = value.parse().map_err(|_| StateError::InvalidValue {
                    field: field.to_string(),
                    value: value.to_string(),
                })?;
            }
            "complexity" => {
                self.complexity = value.parse().map_err(|_| StateError::InvalidValue {
                    field: field.to_string(),
                    value: value.to_string(),
                })?;
            }
            "iteration_count" => {
                self.iteration_count = value.parse().map_err(|_| StateError::InvalidValue {
                    field: field.to_string(),
                    value: value.to_string(),
                })?;
            }
            "failure_count" => {
                self.failure_count = value.parse().map_err(|_| StateError::InvalidValue {
                    field: field.to_string(),
                    value: value.to_string(),
                })?;
            }
            "max_iterations" => {
                self.max_iterations =
                    Some(value.parse().map_err(|_| StateError::InvalidValue {
                        field: field.to_string(),
                        value: value.to_string(),
                    })?);
            }
            "epic_id" => {
                self.epic_id = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "molecule_id" => {
                self.molecule_id = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "current_task" => {
                self.current_task = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "promise_made" => {
                self.promise_made = if value.is_empty() {
                    None
                } else {
                    Some(value.parse().map_err(|_| StateError::InvalidValue {
                        field: field.to_string(),
                        value: value.to_string(),
                    })?)
                };
            }
            "commit_made" => {
                self.commit_made = value.parse().map_err(|_| StateError::InvalidValue {
                    field: field.to_string(),
                    value: value.to_string(),
                })?;
            }
            "tests_ran" => {
                self.tests_ran = value.parse().map_err(|_| StateError::InvalidValue {
                    field: field.to_string(),
                    value: value.to_string(),
                })?;
            }
            "create_pr" => {
                self.create_pr = value.parse().map_err(|_| StateError::InvalidValue {
                    field: field.to_string(),
                    value: value.to_string(),
                })?;
            }
            "worktree_path" => {
                self.worktree_path = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "branch_name" => {
                self.branch_name = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            _ => {
                return Err(StateError::UnknownField(field.to_string()));
            }
        }
        Ok(())
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
    fn test_completion_promise() {
        assert_eq!(WorkflowMode::Planning.completion_promise(), "PLAN_READY");
        assert_eq!(WorkflowMode::Building.completion_promise(), "DONE");
        assert_eq!(WorkflowMode::Paused.completion_promise(), "");
    }

    #[test]
    fn test_is_active() {
        assert!(WorkflowMode::Planning.is_active());
        assert!(WorkflowMode::Building.is_active());
        assert!(!WorkflowMode::Paused.is_active());
        assert!(!WorkflowMode::Complete.is_active());
    }

    #[test]
    fn test_session_state_new() {
        let state = SessionState::new("test-session".to_string());

        assert_eq!(state.session_id, "test-session");
        assert_eq!(state.mode, WorkflowMode::Building);
        assert_eq!(state.complexity, Complexity::Standard);
        assert_eq!(state.iteration_count, 0);
        assert_eq!(state.failure_count, 0);
        assert!(state.epic_id.is_none());
        assert!(state.molecule_id.is_none());
    }

    #[test]
    fn test_session_state_builder() {
        let state = SessionState::new("test".to_string())
            .with_mode(WorkflowMode::Planning)
            .with_epic_id(Some("epic-1".to_string()))
            .with_molecule_id(Some("mol-1".to_string()))
            .with_complexity(Complexity::Critical)
            .with_max_iterations(Some(10));

        assert_eq!(state.mode, WorkflowMode::Planning);
        assert_eq!(state.epic_id, Some("epic-1".to_string()));
        assert_eq!(state.molecule_id, Some("mol-1".to_string()));
        assert_eq!(state.complexity, Complexity::Critical);
        assert_eq!(state.max_iterations, Some(10));
    }

    #[test]
    fn test_should_continue_active_mode() {
        let state = SessionState::new("test".to_string())
            .with_mode(WorkflowMode::Building)
            .with_max_iterations(Some(10));

        assert!(state.should_continue());
    }

    #[test]
    fn test_should_continue_inactive_mode() {
        let state = SessionState::new("test".to_string()).with_mode(WorkflowMode::Paused);

        assert!(!state.should_continue());
    }

    #[test]
    fn test_should_continue_max_iterations() {
        let mut state = SessionState::new("test".to_string())
            .with_mode(WorkflowMode::Building)
            .with_max_iterations(Some(5));

        state.iteration_count = 5;

        assert!(!state.should_continue());
        assert_eq!(state.continuation_reason(), "max_iterations_reached");
    }

    #[test]
    fn test_should_continue_promise_fulfilled() {
        let mut state = SessionState::new("test".to_string()).with_mode(WorkflowMode::Building);

        state.set_promise(PromiseType::Done);

        assert!(!state.should_continue());
        assert_eq!(state.continuation_reason(), "promise_fulfilled");
    }

    #[test]
    fn test_update_field() {
        let mut state = SessionState::new("test".to_string());

        state.update_field("mode", "planning").unwrap();
        assert_eq!(state.mode, WorkflowMode::Planning);

        state.update_field("complexity", "critical").unwrap();
        assert_eq!(state.complexity, Complexity::Critical);

        state.update_field("iteration_count", "5").unwrap();
        assert_eq!(state.iteration_count, 5);

        state.update_field("epic_id", "epic-123").unwrap();
        assert_eq!(state.epic_id, Some("epic-123".to_string()));
    }

    #[test]
    fn test_update_field_unknown() {
        let mut state = SessionState::new("test".to_string());

        let result = state.update_field("unknown_field", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_serialization() {
        let state = SessionState::new("test".to_string())
            .with_mode(WorkflowMode::Planning)
            .with_epic_id(Some("epic-1".to_string()))
            .with_complexity(Complexity::Critical);

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SessionState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.session_id, state.session_id);
        assert_eq!(deserialized.mode, state.mode);
        assert_eq!(deserialized.epic_id, state.epic_id);
        assert_eq!(deserialized.complexity, state.complexity);
    }

    #[test]
    fn test_increment_iteration() {
        let mut state = SessionState::new("test".to_string());

        assert_eq!(state.iteration_count, 0);
        state.increment_iteration();
        assert_eq!(state.iteration_count, 1);
        state.increment_iteration();
        assert_eq!(state.iteration_count, 2);
    }

    #[test]
    fn test_failure_tracking() {
        let mut state = SessionState::new("test".to_string());

        assert_eq!(state.failure_count, 0);
        state.increment_failure();
        assert_eq!(state.failure_count, 1);
        state.increment_failure();
        assert_eq!(state.failure_count, 2);
        state.reset_failures();
        assert_eq!(state.failure_count, 0);
    }

    #[test]
    fn test_add_modified_file() {
        let mut state = SessionState::new("test".to_string());

        state.add_modified_file("file1.rs".to_string());
        state.add_modified_file("file2.rs".to_string());
        state.add_modified_file("file1.rs".to_string()); // duplicate

        assert_eq!(state.files_modified.len(), 2);
        assert!(state.files_modified.contains(&"file1.rs".to_string()));
        assert!(state.files_modified.contains(&"file2.rs".to_string()));
    }
}
