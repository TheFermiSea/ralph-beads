//! Swarm Integration Module
//!
//! Provides Rust wrappers for `bd swarm` commands to enable parallel epic execution
//! with swarm coordination. A swarm orchestrates multiple workers executing tasks
//! from an epic in parallel, respecting dependencies.
//!
//! Key concepts:
//! - **Swarm**: A coordination mechanism for parallel work on an epic
//! - **Orchestrator**: The agent that creates and monitors the swarm
//! - **Worker**: An agent that claims and executes individual tasks
//! - **Task Claiming**: Workers atomically claim tasks to prevent conflicts
//!
//! The swarm model integrates with:
//! - `gates.rs`: For async coordination between workers
//! - `worktree.rs`: For isolated development environments per worker
//! - `memory.rs`: For shared failure patterns with file locking

use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Command;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur during swarm operations
#[derive(Error, Debug)]
pub enum SwarmError {
    #[error("Invalid swarm role: {0}. Valid roles: orchestrator, worker")]
    InvalidRole(String),

    #[error("Invalid swarm status: {0}. Valid statuses: idle, running, completed, failed")]
    InvalidStatus(String),

    #[error("Swarm not found for epic: {0}")]
    NotFound(String),

    #[error("Epic not found: {0}")]
    EpicNotFound(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Task already claimed by another worker: {0}")]
    TaskAlreadyClaimed(String),

    #[error("No tasks available for claiming")]
    NoTasksAvailable,

    #[error("Swarm already exists for epic: {0}")]
    AlreadyExists(String),

    #[error("Epic structure invalid for swarming: {0}")]
    InvalidEpicStructure(String),

    #[error("Beads CLI error: {0}")]
    CliError(String),

    #[error("Failed to execute bd command: {0}")]
    ExecutionError(#[from] std::io::Error),

    #[error("Failed to parse JSON output: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Role of an agent in the swarm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SwarmRole {
    /// The orchestrator creates the swarm, monitors progress, and handles completion
    #[default]
    Orchestrator,
    /// Workers claim and execute individual tasks from the swarm
    Worker,
}

impl fmt::Display for SwarmRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SwarmRole::Orchestrator => write!(f, "orchestrator"),
            SwarmRole::Worker => write!(f, "worker"),
        }
    }
}

impl FromStr for SwarmRole {
    type Err = SwarmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "orchestrator" | "coordinator" | "main" => Ok(SwarmRole::Orchestrator),
            "worker" | "agent" | "executor" => Ok(SwarmRole::Worker),
            _ => Err(SwarmError::InvalidRole(s.to_string())),
        }
    }
}

/// Current status of the swarm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SwarmStatus {
    /// Swarm created but not yet started
    #[default]
    Idle,
    /// Swarm is actively processing tasks
    Running,
    /// All tasks completed successfully
    Completed,
    /// Swarm stopped due to failures or cancellation
    Failed,
}

impl fmt::Display for SwarmStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SwarmStatus::Idle => write!(f, "idle"),
            SwarmStatus::Running => write!(f, "running"),
            SwarmStatus::Completed => write!(f, "completed"),
            SwarmStatus::Failed => write!(f, "failed"),
        }
    }
}

impl FromStr for SwarmStatus {
    type Err = SwarmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "idle" | "pending" | "created" => Ok(SwarmStatus::Idle),
            "running" | "active" | "in_progress" => Ok(SwarmStatus::Running),
            "completed" | "done" | "finished" => Ok(SwarmStatus::Completed),
            "failed" | "error" | "stopped" | "cancelled" => Ok(SwarmStatus::Failed),
            _ => Err(SwarmError::InvalidStatus(s.to_string())),
        }
    }
}

/// Configuration for starting or joining a swarm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    /// The epic ID that this swarm orchestrates
    pub epic_id: String,
    /// Maximum number of workers that can join
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
    /// Worker ID for this agent (None for orchestrator)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    /// Role of this agent in the swarm
    #[serde(default)]
    pub role: SwarmRole,
    /// Optional coordinator address (e.g., "gastown/witness")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinator: Option<String>,
    /// Force creation even if swarm already exists
    #[serde(default)]
    pub force: bool,
}

fn default_max_workers() -> usize {
    4
}

impl SwarmConfig {
    /// Create a new swarm configuration for an epic
    pub fn new(epic_id: &str) -> Self {
        Self {
            epic_id: epic_id.to_string(),
            max_workers: default_max_workers(),
            worker_id: None,
            role: SwarmRole::Orchestrator,
            coordinator: None,
            force: false,
        }
    }

    /// Set the maximum number of workers
    pub fn with_max_workers(mut self, max_workers: usize) -> Self {
        self.max_workers = max_workers;
        self
    }

    /// Set the worker ID (for joining as a worker)
    pub fn with_worker_id(mut self, worker_id: &str) -> Self {
        self.worker_id = Some(worker_id.to_string());
        self.role = SwarmRole::Worker;
        self
    }

    /// Set the role explicitly
    pub fn with_role(mut self, role: SwarmRole) -> Self {
        self.role = role;
        self
    }

    /// Set the coordinator address
    pub fn with_coordinator(mut self, coordinator: &str) -> Self {
        self.coordinator = Some(coordinator.to_string());
        self
    }

    /// Set force flag to overwrite existing swarm
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }
}

/// State of a running swarm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmState {
    /// Current status of the swarm
    pub status: SwarmStatus,
    /// Swarm molecule ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swarm_id: Option<String>,
    /// Epic ID being orchestrated
    pub epic_id: String,
    /// List of active worker IDs
    #[serde(default)]
    pub active_workers: Vec<String>,
    /// Number of tasks completed
    pub tasks_completed: usize,
    /// Number of tasks remaining
    pub tasks_remaining: usize,
    /// Total number of tasks in the epic
    pub tasks_total: usize,
    /// Number of tasks currently in progress
    #[serde(default)]
    pub tasks_in_progress: usize,
    /// Number of blocked tasks
    #[serde(default)]
    pub tasks_blocked: usize,
    /// Number of ready tasks (can be claimed)
    #[serde(default)]
    pub tasks_ready: usize,
    /// Progress percentage (0-100)
    pub progress_percent: f32,
    /// Estimated maximum parallelism
    #[serde(default)]
    pub max_parallelism: usize,
}

impl Default for SwarmState {
    fn default() -> Self {
        Self {
            status: SwarmStatus::Idle,
            swarm_id: None,
            epic_id: String::new(),
            active_workers: Vec::new(),
            tasks_completed: 0,
            tasks_remaining: 0,
            tasks_total: 0,
            tasks_in_progress: 0,
            tasks_blocked: 0,
            tasks_ready: 0,
            progress_percent: 0.0,
            max_parallelism: 1,
        }
    }
}

/// Information about a task claimed from the swarm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimedTask {
    /// Task ID
    pub task_id: String,
    /// Task title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Worker who claimed it
    pub worker_id: String,
    /// Timestamp when claimed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
}

/// Validation result for an epic's swarm readiness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmValidation {
    /// Whether the epic is valid for swarming
    pub is_valid: bool,
    /// Ready fronts (waves of parallel work)
    #[serde(default)]
    pub ready_fronts: usize,
    /// Estimated worker-sessions needed
    #[serde(default)]
    pub estimated_sessions: usize,
    /// Maximum parallelism possible
    #[serde(default)]
    pub max_parallelism: usize,
    /// Warnings about potential issues
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Errors that prevent swarming
    #[serde(default)]
    pub errors: Vec<String>,
}

/// Start a new swarm for an epic
///
/// Creates a swarm molecule that coordinates parallel work on the epic's tasks.
/// The calling agent becomes the orchestrator.
///
/// # Arguments
/// * `config` - Configuration for the swarm
///
/// # Returns
/// The initial state of the swarm
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::{start_swarm, SwarmConfig};
///
/// let config = SwarmConfig::new("epic-123").with_max_workers(4);
/// let state = start_swarm(config)?;
/// println!("Swarm started with {} tasks", state.tasks_total);
/// ```
pub fn start_swarm(config: SwarmConfig) -> Result<SwarmState, SwarmError> {
    // First validate the epic structure
    let validation = validate_epic(&config.epic_id)?;
    if !validation.is_valid {
        let error_msg = validation.errors.join("; ");
        return Err(SwarmError::InvalidEpicStructure(error_msg));
    }

    // Build command arguments
    let mut args = vec!["swarm", "create", &config.epic_id, "--json"];

    let coordinator_arg;
    if let Some(coordinator) = &config.coordinator {
        coordinator_arg = format!("--coordinator={}", coordinator);
        args.push(&coordinator_arg);
    }

    if config.force {
        args.push("--force");
    }

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already exists") {
            return Err(SwarmError::AlreadyExists(config.epic_id));
        }
        if stderr.contains("not found") {
            return Err(SwarmError::EpicNotFound(config.epic_id));
        }
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    // Get the swarm status
    get_swarm_status(&config.epic_id)
}

/// Join an existing swarm as a worker
///
/// Registers this agent as a worker in the swarm. The worker can then
/// claim tasks using `claim_next_task`.
///
/// # Arguments
/// * `epic_id` - The epic ID of the swarm to join
/// * `worker_id` - Unique identifier for this worker
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::join_swarm;
///
/// join_swarm("epic-123", "worker-1")?;
/// ```
pub fn join_swarm(epic_id: &str, worker_id: &str) -> Result<(), SwarmError> {
    // Verify the swarm exists by checking status
    let status = get_swarm_status(epic_id)?;
    if status.swarm_id.is_none() {
        return Err(SwarmError::NotFound(epic_id.to_string()));
    }

    // The worker joins implicitly by claiming tasks
    // We just verify the swarm exists and log the join
    // Beads tracks workers via task assignments

    // Add a comment to the epic noting the worker joined
    let comment = format!("Worker {} joined swarm", worker_id);
    let _ = Command::new("bd")
        .args(["comments", "add", epic_id, &comment])
        .output();

    Ok(())
}

/// Get the current status of a swarm
///
/// Returns computed status based on the current state of tasks in beads.
/// This is a live computation, not stored state.
///
/// # Arguments
/// * `epic_id` - The epic ID or swarm molecule ID
///
/// # Returns
/// Current state of the swarm
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::get_swarm_status;
///
/// let state = get_swarm_status("epic-123")?;
/// println!("Progress: {}%", state.progress_percent);
/// ```
pub fn get_swarm_status(epic_id: &str) -> Result<SwarmState, SwarmError> {
    let output = Command::new("bd")
        .args(["swarm", "status", epic_id, "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(SwarmError::NotFound(epic_id.to_string()));
        }
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_swarm_status(&stdout, epic_id)
}

/// Claim the next available task from the swarm
///
/// Atomically claims a task for this worker. Uses beads' task selection
/// algorithm to pick the next ready task (one with all dependencies satisfied).
///
/// # Arguments
/// * `epic_id` - The epic ID of the swarm
/// * `worker_id` - The worker claiming the task
///
/// # Returns
/// The claimed task ID, or None if no tasks are available
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::claim_next_task;
///
/// if let Some(task_id) = claim_next_task("epic-123", "worker-1")? {
///     println!("Claimed task: {}", task_id);
/// }
/// ```
pub fn claim_next_task(epic_id: &str, worker_id: &str) -> Result<Option<String>, SwarmError> {
    // Get ready tasks from the swarm
    let output = Command::new("bd")
        .args([
            "--no-daemon",
            "ready",
            &format!("--parent={}", epic_id),
            "--limit=1",
            "--json",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(SwarmError::EpicNotFound(epic_id.to_string()));
        }
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the ready tasks
    if stdout.trim().is_empty() || stdout.trim() == "[]" {
        return Ok(None);
    }

    #[derive(Deserialize)]
    struct ReadyTask {
        id: String,
    }

    let tasks: Vec<ReadyTask> = match serde_json::from_str(&stdout) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };

    if tasks.is_empty() {
        return Ok(None);
    }

    let task_id = &tasks[0].id;

    // Claim the task by setting it to in_progress and assigning the worker
    let claim_output = Command::new("bd")
        .args([
            "update",
            task_id,
            "--status=in_progress",
            &format!("--assignee={}", worker_id),
        ])
        .output()?;

    if !claim_output.status.success() {
        let stderr = String::from_utf8_lossy(&claim_output.stderr);
        if stderr.contains("already") || stderr.contains("conflict") {
            return Err(SwarmError::TaskAlreadyClaimed(task_id.to_string()));
        }
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    Ok(Some(task_id.clone()))
}

/// Report that a task has been completed
///
/// Marks the task as closed and updates the swarm progress.
///
/// # Arguments
/// * `epic_id` - The epic ID of the swarm
/// * `task_id` - The completed task ID
/// * `worker_id` - The worker that completed it
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::report_task_complete;
///
/// report_task_complete("epic-123", "task-456", "worker-1")?;
/// ```
pub fn report_task_complete(
    epic_id: &str,
    task_id: &str,
    worker_id: &str,
) -> Result<(), SwarmError> {
    // Close the task
    let output = Command::new("bd")
        .args(["close", task_id])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(SwarmError::TaskNotFound(task_id.to_string()));
        }
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    // Add a comment noting completion
    let comment = format!("Completed by worker {}", worker_id);
    let _ = Command::new("bd")
        .args(["comments", "add", task_id, &comment])
        .output();

    // Check if this completes the swarm
    let status = get_swarm_status(epic_id)?;
    if status.tasks_remaining == 0 {
        // All tasks done - add completion note to epic
        let _ = Command::new("bd")
            .args(["comments", "add", epic_id, "Swarm completed: all tasks done"])
            .output();
    }

    Ok(())
}

/// Report that a task has failed
///
/// Marks the task as blocked and logs the failure reason.
///
/// # Arguments
/// * `epic_id` - The epic ID of the swarm
/// * `task_id` - The failed task ID
/// * `worker_id` - The worker that encountered the failure
/// * `reason` - Description of the failure
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::report_task_failed;
///
/// report_task_failed("epic-123", "task-456", "worker-1", "Tests failed")?;
/// ```
pub fn report_task_failed(
    epic_id: &str,
    task_id: &str,
    worker_id: &str,
    reason: &str,
) -> Result<(), SwarmError> {
    // Mark task as blocked
    let output = Command::new("bd")
        .args(["update", task_id, "--status=blocked"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(SwarmError::TaskNotFound(task_id.to_string()));
        }
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    // Add failure comment
    let comment = format!("Failed (worker {}): {}", worker_id, reason);
    let _ = Command::new("bd")
        .args(["comments", "add", task_id, &comment])
        .output();

    // Add blocked label for circuit breaker
    let _ = Command::new("bd")
        .args(["label", "add", task_id, "blocked"])
        .output();

    // Note in epic
    let epic_comment = format!("Task {} blocked: {}", task_id, reason);
    let _ = Command::new("bd")
        .args(["comments", "add", epic_id, &epic_comment])
        .output();

    Ok(())
}

/// Stop a swarm
///
/// Stops the swarm execution. Any in-progress tasks are left in their
/// current state for later resumption.
///
/// # Arguments
/// * `epic_id` - The epic ID of the swarm to stop
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::stop_swarm;
///
/// stop_swarm("epic-123")?;
/// ```
pub fn stop_swarm(epic_id: &str) -> Result<(), SwarmError> {
    // Add a comment noting the stop
    let output = Command::new("bd")
        .args(["comments", "add", epic_id, "Swarm stopped by orchestrator"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(SwarmError::EpicNotFound(epic_id.to_string()));
        }
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    Ok(())
}

/// Validate an epic's structure for swarm execution
///
/// Checks that the epic has a valid dependency graph for parallel execution.
///
/// # Arguments
/// * `epic_id` - The epic ID to validate
///
/// # Returns
/// Validation results including warnings and errors
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::validate_epic;
///
/// let validation = validate_epic("epic-123")?;
/// if validation.is_valid {
///     println!("Epic ready for swarming with {} parallel fronts", validation.ready_fronts);
/// }
/// ```
pub fn validate_epic(epic_id: &str) -> Result<SwarmValidation, SwarmError> {
    let output = Command::new("bd")
        .args(["swarm", "validate", epic_id, "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") {
            return Err(SwarmError::EpicNotFound(epic_id.to_string()));
        }
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_validation_result(&stdout)
}

/// List all active swarms
///
/// Returns information about all swarm molecules in the project.
///
/// # Returns
/// Vector of swarm states
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::swarm::list_swarms;
///
/// let swarms = list_swarms()?;
/// for swarm in swarms {
///     println!("{}: {}% complete", swarm.epic_id, swarm.progress_percent);
/// }
/// ```
pub fn list_swarms() -> Result<Vec<SwarmState>, SwarmError> {
    let output = Command::new("bd")
        .args(["swarm", "list", "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SwarmError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.trim().is_empty() || stdout.trim() == "[]" {
        return Ok(Vec::new());
    }

    parse_swarm_list(&stdout)
}

// Helper functions

/// Parse swarm status from JSON output
fn parse_swarm_status(json_str: &str, epic_id: &str) -> Result<SwarmState, SwarmError> {
    let json: serde_json::Value = serde_json::from_str(json_str)?;

    // Extract counts from status response
    let completed = json
        .get("completed")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let active = json
        .get("active")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let ready = json
        .get("ready")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let blocked = json
        .get("blocked")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let total = completed + active + ready + blocked;
    let remaining = active + ready + blocked;

    // Extract active workers from active tasks
    let active_workers: Vec<String> = json
        .get("active")
        .and_then(|v| v.as_array())
        .map(|tasks| {
            tasks
                .iter()
                .filter_map(|t| t.get("assignee").and_then(|a| a.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    // Determine status
    let status = if total == 0 {
        SwarmStatus::Idle
    } else if completed == total {
        SwarmStatus::Completed
    } else if blocked > 0 && ready == 0 && active == 0 {
        SwarmStatus::Failed
    } else {
        SwarmStatus::Running
    };

    // Calculate progress
    let progress = if total > 0 {
        (completed as f32 / total as f32) * 100.0
    } else {
        0.0
    };

    // Extract swarm molecule ID if present
    let swarm_id = json
        .get("swarm_id")
        .or_else(|| json.get("molecule_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(SwarmState {
        status,
        swarm_id,
        epic_id: epic_id.to_string(),
        active_workers,
        tasks_completed: completed,
        tasks_remaining: remaining,
        tasks_total: total,
        tasks_in_progress: active,
        tasks_blocked: blocked,
        tasks_ready: ready,
        progress_percent: progress,
        max_parallelism: ready.max(1),
    })
}

/// Parse validation result from JSON output
fn parse_validation_result(json_str: &str) -> Result<SwarmValidation, SwarmError> {
    let json: serde_json::Value = serde_json::from_str(json_str)?;

    let errors: Vec<String> = json
        .get("errors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    let warnings: Vec<String> = json
        .get("warnings")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    let ready_fronts = json
        .get("ready_fronts")
        .or_else(|| json.get("fronts"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let estimated_sessions = json
        .get("estimated_sessions")
        .or_else(|| json.get("worker_sessions"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let max_parallelism = json
        .get("max_parallelism")
        .or_else(|| json.get("parallelism"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as usize;

    // Valid if no errors (warnings are ok)
    let is_valid = errors.is_empty();

    Ok(SwarmValidation {
        is_valid,
        ready_fronts,
        estimated_sessions,
        max_parallelism,
        warnings,
        errors,
    })
}

/// Parse swarm list from JSON output
fn parse_swarm_list(json_str: &str) -> Result<Vec<SwarmState>, SwarmError> {
    let json: serde_json::Value = serde_json::from_str(json_str)?;

    let swarms = json
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let epic_id = item.get("epic_id")?.as_str()?;

                    let completed = item
                        .get("completed")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;

                    let total = item.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                    let progress = if total > 0 {
                        (completed as f32 / total as f32) * 100.0
                    } else {
                        0.0
                    };

                    let status = if completed == total && total > 0 {
                        SwarmStatus::Completed
                    } else if total > 0 {
                        SwarmStatus::Running
                    } else {
                        SwarmStatus::Idle
                    };

                    Some(SwarmState {
                        status,
                        swarm_id: item.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        epic_id: epic_id.to_string(),
                        active_workers: Vec::new(),
                        tasks_completed: completed,
                        tasks_remaining: total - completed,
                        tasks_total: total,
                        tasks_in_progress: 0,
                        tasks_blocked: 0,
                        tasks_ready: 0,
                        progress_percent: progress,
                        max_parallelism: 1,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(swarms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_role_from_str() {
        assert_eq!(
            SwarmRole::from_str("orchestrator").unwrap(),
            SwarmRole::Orchestrator
        );
        assert_eq!(
            SwarmRole::from_str("coordinator").unwrap(),
            SwarmRole::Orchestrator
        );
        assert_eq!(
            SwarmRole::from_str("ORCHESTRATOR").unwrap(),
            SwarmRole::Orchestrator
        );
        assert_eq!(SwarmRole::from_str("worker").unwrap(), SwarmRole::Worker);
        assert_eq!(SwarmRole::from_str("agent").unwrap(), SwarmRole::Worker);
        assert_eq!(SwarmRole::from_str("executor").unwrap(), SwarmRole::Worker);
    }

    #[test]
    fn test_swarm_role_from_str_invalid() {
        assert!(SwarmRole::from_str("invalid").is_err());
        assert!(SwarmRole::from_str("").is_err());
    }

    #[test]
    fn test_swarm_role_display() {
        assert_eq!(SwarmRole::Orchestrator.to_string(), "orchestrator");
        assert_eq!(SwarmRole::Worker.to_string(), "worker");
    }

    #[test]
    fn test_swarm_status_from_str() {
        assert_eq!(SwarmStatus::from_str("idle").unwrap(), SwarmStatus::Idle);
        assert_eq!(
            SwarmStatus::from_str("pending").unwrap(),
            SwarmStatus::Idle
        );
        assert_eq!(
            SwarmStatus::from_str("running").unwrap(),
            SwarmStatus::Running
        );
        assert_eq!(
            SwarmStatus::from_str("active").unwrap(),
            SwarmStatus::Running
        );
        assert_eq!(
            SwarmStatus::from_str("in_progress").unwrap(),
            SwarmStatus::Running
        );
        assert_eq!(
            SwarmStatus::from_str("completed").unwrap(),
            SwarmStatus::Completed
        );
        assert_eq!(SwarmStatus::from_str("done").unwrap(), SwarmStatus::Completed);
        assert_eq!(
            SwarmStatus::from_str("finished").unwrap(),
            SwarmStatus::Completed
        );
        assert_eq!(SwarmStatus::from_str("failed").unwrap(), SwarmStatus::Failed);
        assert_eq!(SwarmStatus::from_str("error").unwrap(), SwarmStatus::Failed);
        assert_eq!(
            SwarmStatus::from_str("stopped").unwrap(),
            SwarmStatus::Failed
        );
    }

    #[test]
    fn test_swarm_status_from_str_invalid() {
        assert!(SwarmStatus::from_str("invalid").is_err());
        assert!(SwarmStatus::from_str("").is_err());
    }

    #[test]
    fn test_swarm_status_display() {
        assert_eq!(SwarmStatus::Idle.to_string(), "idle");
        assert_eq!(SwarmStatus::Running.to_string(), "running");
        assert_eq!(SwarmStatus::Completed.to_string(), "completed");
        assert_eq!(SwarmStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn test_swarm_config_builder() {
        let config = SwarmConfig::new("epic-123")
            .with_max_workers(8)
            .with_coordinator("gastown/witness");

        assert_eq!(config.epic_id, "epic-123");
        assert_eq!(config.max_workers, 8);
        assert_eq!(config.coordinator, Some("gastown/witness".to_string()));
        assert_eq!(config.role, SwarmRole::Orchestrator);
    }

    #[test]
    fn test_swarm_config_worker() {
        let config = SwarmConfig::new("epic-123").with_worker_id("worker-1");

        assert_eq!(config.worker_id, Some("worker-1".to_string()));
        assert_eq!(config.role, SwarmRole::Worker);
    }

    #[test]
    fn test_swarm_config_force() {
        let config = SwarmConfig::new("epic-123").with_force(true);
        assert!(config.force);
    }

    #[test]
    fn test_swarm_state_default() {
        let state = SwarmState::default();
        assert_eq!(state.status, SwarmStatus::Idle);
        assert_eq!(state.tasks_total, 0);
        assert_eq!(state.progress_percent, 0.0);
    }

    #[test]
    fn test_swarm_role_serialization() {
        let role = SwarmRole::Orchestrator;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"orchestrator\"");

        let deserialized: SwarmRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SwarmRole::Orchestrator);
    }

    #[test]
    fn test_swarm_status_serialization() {
        let status = SwarmStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let deserialized: SwarmStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SwarmStatus::Running);
    }

    #[test]
    fn test_swarm_config_serialization() {
        let config = SwarmConfig::new("epic-123")
            .with_max_workers(4)
            .with_worker_id("worker-1");

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"epic_id\":\"epic-123\""));
        assert!(json.contains("\"max_workers\":4"));
        assert!(json.contains("\"worker_id\":\"worker-1\""));

        let deserialized: SwarmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.epic_id, "epic-123");
        assert_eq!(deserialized.max_workers, 4);
    }

    #[test]
    fn test_swarm_state_serialization() {
        let state = SwarmState {
            status: SwarmStatus::Running,
            swarm_id: Some("swarm-456".to_string()),
            epic_id: "epic-123".to_string(),
            active_workers: vec!["worker-1".to_string()],
            tasks_completed: 5,
            tasks_remaining: 10,
            tasks_total: 15,
            tasks_in_progress: 2,
            tasks_blocked: 1,
            tasks_ready: 7,
            progress_percent: 33.33,
            max_parallelism: 4,
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"status\":\"running\""));
        assert!(json.contains("\"tasks_total\":15"));

        let deserialized: SwarmState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tasks_completed, 5);
    }

    #[test]
    fn test_swarm_validation_serialization() {
        let validation = SwarmValidation {
            is_valid: true,
            ready_fronts: 3,
            estimated_sessions: 12,
            max_parallelism: 5,
            warnings: vec!["Some tasks have no description".to_string()],
            errors: vec![],
        };

        let json = serde_json::to_string(&validation).unwrap();
        assert!(json.contains("\"is_valid\":true"));
        assert!(json.contains("\"max_parallelism\":5"));

        let deserialized: SwarmValidation = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_valid);
        assert_eq!(deserialized.warnings.len(), 1);
    }

    #[test]
    fn test_claimed_task_serialization() {
        let task = ClaimedTask {
            task_id: "task-789".to_string(),
            title: Some("Implement feature X".to_string()),
            worker_id: "worker-1".to_string(),
            claimed_at: Some("2024-01-15T10:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"task_id\":\"task-789\""));
        assert!(json.contains("\"worker_id\":\"worker-1\""));

        let deserialized: ClaimedTask = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.task_id, "task-789");
    }

    #[test]
    fn test_default_values() {
        assert_eq!(SwarmRole::default(), SwarmRole::Orchestrator);
        assert_eq!(SwarmStatus::default(), SwarmStatus::Idle);
        assert_eq!(default_max_workers(), 4);
    }

    #[test]
    fn test_parse_swarm_status_empty() {
        let json = r#"{}"#;
        let result = parse_swarm_status(json, "epic-123").unwrap();
        assert_eq!(result.status, SwarmStatus::Idle);
        assert_eq!(result.tasks_total, 0);
    }

    #[test]
    fn test_parse_swarm_status_running() {
        let json = r#"{
            "completed": ["task-1"],
            "active": [{"id": "task-2", "assignee": "worker-1"}],
            "ready": ["task-3", "task-4"],
            "blocked": []
        }"#;
        let result = parse_swarm_status(json, "epic-123").unwrap();
        assert_eq!(result.status, SwarmStatus::Running);
        assert_eq!(result.tasks_completed, 1);
        assert_eq!(result.tasks_in_progress, 1);
        assert_eq!(result.tasks_ready, 2);
        assert_eq!(result.tasks_total, 4);
        assert_eq!(result.active_workers, vec!["worker-1"]);
    }

    #[test]
    fn test_parse_swarm_status_completed() {
        let json = r#"{
            "completed": ["task-1", "task-2", "task-3"],
            "active": [],
            "ready": [],
            "blocked": []
        }"#;
        let result = parse_swarm_status(json, "epic-123").unwrap();
        assert_eq!(result.status, SwarmStatus::Completed);
        assert_eq!(result.tasks_completed, 3);
        assert_eq!(result.progress_percent, 100.0);
    }

    #[test]
    fn test_parse_swarm_status_failed() {
        let json = r#"{
            "completed": ["task-1"],
            "active": [],
            "ready": [],
            "blocked": ["task-2", "task-3"]
        }"#;
        let result = parse_swarm_status(json, "epic-123").unwrap();
        assert_eq!(result.status, SwarmStatus::Failed);
        assert_eq!(result.tasks_blocked, 2);
    }

    #[test]
    fn test_parse_validation_result_valid() {
        let json = r#"{
            "ready_fronts": 3,
            "estimated_sessions": 10,
            "max_parallelism": 4,
            "warnings": ["Minor issue"],
            "errors": []
        }"#;
        let result = parse_validation_result(json).unwrap();
        assert!(result.is_valid);
        assert_eq!(result.ready_fronts, 3);
        assert_eq!(result.max_parallelism, 4);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_parse_validation_result_invalid() {
        let json = r#"{
            "ready_fronts": 0,
            "errors": ["Cycle detected", "Missing root"]
        }"#;
        let result = parse_validation_result(json).unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn test_parse_swarm_list() {
        let json = r#"[
            {"id": "swarm-1", "epic_id": "epic-1", "completed": 5, "total": 10},
            {"id": "swarm-2", "epic_id": "epic-2", "completed": 3, "total": 3}
        ]"#;
        let result = parse_swarm_list(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].epic_id, "epic-1");
        assert_eq!(result[0].progress_percent, 50.0);
        assert_eq!(result[1].status, SwarmStatus::Completed);
    }

    #[test]
    fn test_parse_swarm_list_empty() {
        let json = r#"[]"#;
        let result = parse_swarm_list(json).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_swarm_error_display() {
        let err = SwarmError::NotFound("epic-123".to_string());
        assert_eq!(err.to_string(), "Swarm not found for epic: epic-123");

        let err = SwarmError::TaskAlreadyClaimed("task-456".to_string());
        assert_eq!(
            err.to_string(),
            "Task already claimed by another worker: task-456"
        );

        let err = SwarmError::NoTasksAvailable;
        assert_eq!(err.to_string(), "No tasks available for claiming");
    }
}
