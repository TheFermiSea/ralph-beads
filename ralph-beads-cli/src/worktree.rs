//! Worktree Integration Module
//!
//! Provides Rust wrappers for `bd worktree` commands, enabling isolated development
//! with proper beads database sharing. Worktrees allow multiple working directories
//! sharing the same git repository, enabling parallel development (e.g., multiple
//! agents or features).
//!
//! When creating a worktree via beads, a redirect file is automatically set up so
//! all worktrees share the same .beads database, ensuring consistent issue state.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Command;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur during worktree operations
#[derive(Error, Debug)]
pub enum WorktreeError {
    #[error("Worktree not found: {0}")]
    NotFound(String),

    #[error("Worktree already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid worktree status: {0}")]
    InvalidStatus(String),

    #[error("Worktree has uncommitted changes")]
    UncommittedChanges,

    #[error("Worktree has unpushed commits")]
    UnpushedCommits,

    #[error("Beads CLI error: {0}")]
    CliError(String),

    #[error("Failed to execute bd command: {0}")]
    ExecutionError(#[from] std::io::Error),

    #[error("Failed to parse JSON output: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Beads configuration state for a worktree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BeadsState {
    /// This is the main repository, beads database is shared from here
    Shared,
    /// This worktree redirects to the main repository's beads database
    Redirect,
    /// This worktree has its own local beads database (not shared)
    Local,
    /// No beads configuration present
    None,
}

impl fmt::Display for BeadsState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BeadsState::Shared => write!(f, "shared"),
            BeadsState::Redirect => write!(f, "redirect"),
            BeadsState::Local => write!(f, "local"),
            BeadsState::None => write!(f, "none"),
        }
    }
}

impl FromStr for BeadsState {
    type Err = WorktreeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "shared" => Ok(BeadsState::Shared),
            "redirect" => Ok(BeadsState::Redirect),
            "local" => Ok(BeadsState::Local),
            "none" => Ok(BeadsState::None),
            _ => Ok(BeadsState::None), // Default to none for unknown states
        }
    }
}

/// Information about a git worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    /// Name of the worktree (directory name)
    pub name: String,
    /// Full path to the worktree
    pub path: String,
    /// Branch checked out in this worktree
    pub branch: String,
    /// Git commit hash at HEAD
    #[serde(default)]
    pub commit: String,
    /// Whether this is the main worktree
    pub is_main: bool,
    /// Whether beads database is shared with main repository
    #[serde(default)]
    pub beads_shared: bool,
    /// Beads configuration state
    #[serde(default, rename = "beads_state")]
    beads_state_str: Option<String>,
}

impl WorktreeInfo {
    /// Get the beads state for this worktree
    pub fn beads_state(&self) -> BeadsState {
        self.beads_state_str
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| {
                if self.is_main {
                    BeadsState::Shared
                } else if self.beads_shared {
                    BeadsState::Redirect
                } else {
                    BeadsState::None
                }
            })
    }
}

/// Status information about a worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeStatus {
    /// Full path to the worktree
    pub path: String,
    /// Branch checked out in this worktree
    pub branch: String,
    /// Whether the worktree is clean (no uncommitted changes)
    pub is_clean: bool,
    /// Whether HEAD is detached
    #[serde(default)]
    pub is_detached: bool,
}

/// Information about the current worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentWorktreeInfo {
    /// Whether we are in a worktree (not main repo)
    #[serde(default)]
    pub is_worktree: bool,
    /// Full path to the worktree
    #[serde(default)]
    pub path: Option<String>,
    /// Name of the worktree (directory name)
    #[serde(default)]
    pub name: Option<String>,
    /// Branch checked out in this worktree
    #[serde(default)]
    pub branch: Option<String>,
    /// Whether this is the main worktree
    #[serde(default)]
    pub is_main: bool,
    /// Path to the main repository
    #[serde(default)]
    pub main_repo: Option<String>,
    /// Beads configuration state
    #[serde(default)]
    pub beads_state: Option<String>,
}

/// Create a new worktree with beads redirect configuration
///
/// This creates a git worktree and sets up beads to share the database
/// with the main repository.
///
/// # Arguments
/// * `name` - Name for the worktree (used for identification)
/// * `branch` - Branch name for the worktree
/// * `path` - Path for the worktree directory
///
/// # Returns
/// Information about the created worktree
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::worktree::create_worktree;
/// let info = create_worktree("feature-auth", "feature-auth", "./feature-auth")?;
/// println!("Created worktree at: {}", info.path);
/// ```
pub fn create_worktree(name: &str, branch: &str, _path: &str) -> Result<WorktreeInfo, WorktreeError> {
    let branch_arg = format!("--branch={}", branch);
    let args = vec!["worktree", "create", name, &branch_arg, "--json"];

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already exists") {
            return Err(WorktreeError::AlreadyExists(name.to_string()));
        }
        return Err(WorktreeError::CliError(stderr.to_string()));
    }

    // After creation, get the worktree info
    // The create command may not return full info, so we list and find it
    let worktrees = list_worktrees()?;

    // Find the worktree we just created
    let created = worktrees
        .into_iter()
        .find(|w| w.name == name || w.branch == branch)
        .ok_or_else(|| {
            WorktreeError::CliError("Worktree created but not found in list".to_string())
        })?;

    Ok(created)
}

/// List all worktrees in the repository
///
/// Returns information about all worktrees, including the main worktree
/// and all linked worktrees.
///
/// # Returns
/// Vector of worktree information
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::worktree::list_worktrees;
/// let worktrees = list_worktrees()?;
/// for wt in worktrees {
///     println!("{}: {} ({})", wt.name, wt.path, wt.branch);
/// }
/// ```
pub fn list_worktrees() -> Result<Vec<WorktreeInfo>, WorktreeError> {
    let output = Command::new("bd")
        .args(["worktree", "list", "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let worktrees: Vec<WorktreeInfo> = serde_json::from_str(&stdout)?;

    Ok(worktrees)
}

/// Remove a worktree with safety checks
///
/// By default, this performs safety checks before removal:
/// - Uncommitted changes
/// - Unpushed commits
/// - Stashes
///
/// Use `force = true` to skip safety checks (not recommended).
///
/// # Arguments
/// * `name_or_path` - Name or path of the worktree to remove
/// * `force` - Skip safety checks if true
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::worktree::remove_worktree;
/// remove_worktree("feature-auth", false)?;
/// ```
pub fn remove_worktree(name_or_path: &str, force: bool) -> Result<(), WorktreeError> {
    let mut args = vec!["worktree", "remove", name_or_path];

    if force {
        args.push("--force");
    }

    let output = Command::new("bd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("not found") || stderr.contains("does not exist") {
            return Err(WorktreeError::NotFound(name_or_path.to_string()));
        }
        if stderr.contains("uncommitted") || stderr.contains("changes") {
            return Err(WorktreeError::UncommittedChanges);
        }
        if stderr.contains("unpushed") {
            return Err(WorktreeError::UnpushedCommits);
        }

        return Err(WorktreeError::CliError(stderr.to_string()));
    }

    Ok(())
}

/// Get the status of a specific worktree
///
/// Checks whether the worktree has uncommitted changes, is on a detached HEAD, etc.
///
/// # Arguments
/// * `name_or_path` - Name or path of the worktree to check
///
/// # Returns
/// Status information about the worktree
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::worktree::get_worktree_status;
/// let status = get_worktree_status("feature-auth")?;
/// if !status.is_clean {
///     println!("Worktree has uncommitted changes");
/// }
/// ```
pub fn get_worktree_status(name_or_path: &str) -> Result<WorktreeStatus, WorktreeError> {
    // First, find the worktree to get its path
    let worktrees = list_worktrees()?;
    let worktree = worktrees
        .iter()
        .find(|w| {
            w.name == name_or_path || w.path == name_or_path || w.path.ends_with(name_or_path)
        })
        .ok_or_else(|| WorktreeError::NotFound(name_or_path.to_string()))?;

    // Check if HEAD is detached
    let head_output = Command::new("git")
        .args(["-C", &worktree.path, "symbolic-ref", "-q", "HEAD"])
        .output()?;

    let is_detached = !head_output.status.success();

    // Check git status in that worktree
    let status_output = Command::new("git")
        .args(["-C", &worktree.path, "status", "--porcelain"])
        .output()?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(WorktreeError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&status_output.stdout);
    let is_clean = stdout.trim().is_empty();

    Ok(WorktreeStatus {
        path: worktree.path.clone(),
        branch: worktree.branch.clone(),
        is_clean,
        is_detached,
    })
}

/// Get information about the current worktree
///
/// Returns detailed information about the worktree containing the
/// current working directory.
///
/// # Returns
/// Information about the current worktree
///
/// # Example
/// ```ignore
/// use ralph_beads_cli::worktree::get_current_worktree_info;
/// let info = get_current_worktree_info()?;
/// println!("In worktree: {}", info.path);
/// ```
pub fn get_current_worktree_info() -> Result<CurrentWorktreeInfo, WorktreeError> {
    let output = Command::new("bd")
        .args(["worktree", "info", "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::CliError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Try to parse the JSON response
    #[derive(Deserialize)]
    struct InfoResponse {
        #[serde(default)]
        is_worktree: bool,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        branch: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        main_repo: Option<String>,
        #[serde(default)]
        beads_state: Option<String>,
    }

    let response: InfoResponse = serde_json::from_str(&stdout)?;

    // If not in a worktree, get info from the main repo
    if !response.is_worktree {
        // Get current directory info
        let pwd_output = Command::new("pwd").output()?;
        let path = String::from_utf8_lossy(&pwd_output.stdout).trim().to_string();

        // Get the directory name as the worktree name
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Get current branch
        let branch_output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()?;
        let branch = String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string();

        return Ok(CurrentWorktreeInfo {
            is_worktree: false,
            path: Some(path),
            name: Some(name),
            branch: Some(branch),
            is_main: true,
            main_repo: None,
            beads_state: Some("shared".to_string()),
        });
    }

    Ok(CurrentWorktreeInfo {
        is_worktree: response.is_worktree,
        path: response.path,
        name: response.name,
        branch: response.branch,
        is_main: false,
        main_repo: response.main_repo,
        beads_state: response.beads_state,
    })
}

/// Find a worktree by name or path
///
/// # Arguments
/// * `name_or_path` - Name or path to search for
///
/// # Returns
/// The matching worktree info if found
pub fn find_worktree(name_or_path: &str) -> Result<Option<WorktreeInfo>, WorktreeError> {
    let worktrees = list_worktrees()?;
    Ok(worktrees.into_iter().find(|w| {
        w.name == name_or_path || w.path == name_or_path || w.path.ends_with(name_or_path)
    }))
}

/// Check if currently in a worktree (not the main repository)
///
/// # Returns
/// True if in a linked worktree, false if in main repository or not in a git repo
pub fn is_in_worktree() -> Result<bool, WorktreeError> {
    let info = get_current_worktree_info()?;
    Ok(info.is_worktree)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beads_state_from_str() {
        assert_eq!(BeadsState::from_str("shared").unwrap(), BeadsState::Shared);
        assert_eq!(
            BeadsState::from_str("redirect").unwrap(),
            BeadsState::Redirect
        );
        assert_eq!(BeadsState::from_str("local").unwrap(), BeadsState::Local);
        assert_eq!(BeadsState::from_str("none").unwrap(), BeadsState::None);
        // Unknown values default to None
        assert_eq!(BeadsState::from_str("unknown").unwrap(), BeadsState::None);
    }

    #[test]
    fn test_beads_state_display() {
        assert_eq!(BeadsState::Shared.to_string(), "shared");
        assert_eq!(BeadsState::Redirect.to_string(), "redirect");
        assert_eq!(BeadsState::Local.to_string(), "local");
        assert_eq!(BeadsState::None.to_string(), "none");
    }

    #[test]
    fn test_worktree_info_beads_state() {
        // Test with explicit beads_state
        let info = WorktreeInfo {
            name: "test".to_string(),
            path: "/test".to_string(),
            branch: "main".to_string(),
            commit: "abc123".to_string(),
            is_main: false,
            beads_shared: false,
            beads_state_str: Some("redirect".to_string()),
        };
        assert_eq!(info.beads_state(), BeadsState::Redirect);

        // Test fallback for main worktree
        let main_info = WorktreeInfo {
            name: "main".to_string(),
            path: "/main".to_string(),
            branch: "main".to_string(),
            commit: "abc123".to_string(),
            is_main: true,
            beads_shared: false,
            beads_state_str: None,
        };
        assert_eq!(main_info.beads_state(), BeadsState::Shared);

        // Test fallback for shared worktree
        let shared_info = WorktreeInfo {
            name: "feature".to_string(),
            path: "/feature".to_string(),
            branch: "feature".to_string(),
            commit: "abc123".to_string(),
            is_main: false,
            beads_shared: true,
            beads_state_str: None,
        };
        assert_eq!(shared_info.beads_state(), BeadsState::Redirect);
    }

    #[test]
    fn test_worktree_status_serialization() {
        let status = WorktreeStatus {
            path: "/test/path".to_string(),
            branch: "main".to_string(),
            is_clean: true,
            is_detached: false,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"path\":\"/test/path\""));
        assert!(json.contains("\"is_clean\":true"));

        let deserialized: WorktreeStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.path, "/test/path");
        assert!(deserialized.is_clean);
    }

    #[test]
    fn test_worktree_info_serialization() {
        let info = WorktreeInfo {
            name: "feature".to_string(),
            path: "/path/to/feature".to_string(),
            branch: "feature-branch".to_string(),
            commit: "abc123def".to_string(),
            is_main: false,
            beads_shared: true,
            beads_state_str: Some("redirect".to_string()),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"feature\""));
        assert!(json.contains("\"beads_shared\":true"));

        let deserialized: WorktreeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "feature");
        assert!(deserialized.beads_shared);
    }

    #[test]
    fn test_current_worktree_info_serialization() {
        let info = CurrentWorktreeInfo {
            is_worktree: true,
            path: Some("/path/to/worktree".to_string()),
            name: Some("feature".to_string()),
            branch: Some("feature-branch".to_string()),
            is_main: false,
            main_repo: Some("/path/to/main".to_string()),
            beads_state: Some("redirect".to_string()),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"is_worktree\":true"));
        assert!(json.contains("\"is_main\":false"));

        let deserialized: CurrentWorktreeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.path, Some("/path/to/worktree".to_string()));
        assert!(deserialized.is_worktree);
        assert!(!deserialized.is_main);
    }

    #[test]
    fn test_worktree_error_display() {
        let err = WorktreeError::NotFound("test".to_string());
        assert_eq!(err.to_string(), "Worktree not found: test");

        let err = WorktreeError::AlreadyExists("test".to_string());
        assert_eq!(err.to_string(), "Worktree already exists: test");

        let err = WorktreeError::UncommittedChanges;
        assert_eq!(err.to_string(), "Worktree has uncommitted changes");

        let err = WorktreeError::UnpushedCommits;
        assert_eq!(err.to_string(), "Worktree has unpushed commits");
    }
}
