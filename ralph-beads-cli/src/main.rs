mod activity;
mod beads_state;
mod complexity;
mod framework;
mod gates;
mod health;
mod iterations;
mod lint;
mod memory;
mod preflight;
mod security;
mod state;
mod swarm;
mod worktree;

use clap::{Parser, Subcommand};
use serde_json::json;

use activity::{
    format_activity_text, get_activity_for_issue, get_activity_since, get_recent_activity,
    stream_activity,
};
use complexity::{detect_complexity, Complexity};
use framework::detect_framework;
use health::HealthChecker;
use iterations::calculate_max_iterations;
use memory::ProceduralMemory;
use security::SecurityValidator;
use state::{SessionState, WorkflowMode};

#[derive(Parser)]
#[command(name = "ralph-beads-cli")]
#[command(about = "Rust CLI helper for ralph-beads plugin", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Detect complexity level from task description
    DetectComplexity {
        /// Task description to analyze
        #[arg(short, long)]
        task: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Detect test framework from current directory
    DetectFramework {
        /// Directory to check (defaults to current)
        #[arg(short, long)]
        dir: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Calculate max iterations based on mode and complexity
    CalcIterations {
        /// Workflow mode: planning or building
        #[arg(short, long)]
        mode: String,

        /// Complexity level: trivial, simple, standard, critical
        #[arg(short, long)]
        complexity: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Initialize or load session state
    State {
        #[command(subcommand)]
        action: StateAction,
    },

    /// Output information about all capabilities
    Info {
        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Run health checks before execution
    Health {
        /// Project directory to check
        #[arg(short, long)]
        dir: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Validate a command against security rules
    Validate {
        /// Command to validate
        #[arg(short, long)]
        command: String,

        /// Project root for path validation
        #[arg(short, long)]
        project_root: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Memory operations for failure tracking
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },

    /// Beads state dimension operations (wraps bd set-state/state)
    BeadsState {
        #[command(subcommand)]
        action: BeadsStateAction,
    },

    /// Activity feed operations for real-time progress monitoring
    Activity {
        #[command(subcommand)]
        action: ActivityAction,
    },

    /// Preflight checks for pre-PR validation (wraps bd preflight)
    Preflight {
        #[command(subcommand)]
        action: PreflightAction,
    },

    /// Worktree operations for isolated development with shared beads database
    Worktree {
        #[command(subcommand)]
        action: WorktreeAction,
    },

    /// Gate operations for async coordination (wraps bd gate)
    Gate {
        #[command(subcommand)]
        action: GateAction,
    },

    /// Swarm operations for parallel epic execution (wraps bd swarm)
    Swarm {
        #[command(subcommand)]
        action: SwarmAction,
    },

    /// Lint operations for issue quality checks (wraps bd lint)
    Lint {
        #[command(subcommand)]
        action: LintAction,
    },
}

#[derive(Subcommand)]
enum LintAction {
    /// Lint a single issue by ID
    Issue {
        /// Issue ID to lint
        issue_id: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Lint an epic and all its children
    Epic {
        /// Epic ID to lint
        epic_id: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Lint all open issues in the project
    All {
        /// Minimum severity to report: error, warning, or all
        #[arg(short, long, default_value = "all")]
        severity: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Check if an issue has acceptance criteria
    CheckAc {
        /// Issue ID to check
        issue_id: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Record a success entry
    Success {
        /// Memory log file path
        #[arg(short, long)]
        log_file: String,

        /// Task ID
        #[arg(short, long)]
        task_id: String,

        /// Description
        #[arg(short, long)]
        description: String,

        /// Context (JSON object)
        #[arg(short, long)]
        context: Option<String>,
    },

    /// Record a failure entry
    Failure {
        /// Memory log file path
        #[arg(short, long)]
        log_file: String,

        /// Task ID
        #[arg(short, long)]
        task_id: String,

        /// Error message
        #[arg(short, long)]
        error: String,

        /// Context (JSON object)
        #[arg(short, long)]
        context: Option<String>,
    },

    /// Record a workaround entry
    Workaround {
        /// Memory log file path
        #[arg(short, long)]
        log_file: String,

        /// Task ID
        #[arg(short, long)]
        task_id: String,

        /// Workaround description
        #[arg(short, long)]
        description: String,

        /// Original error that led to workaround
        #[arg(short, long)]
        original_error: Option<String>,
    },

    /// Get failures for a specific task
    GetFailures {
        /// Memory log file path
        #[arg(short, long)]
        log_file: String,

        /// Task ID to get failures for
        #[arg(short, long)]
        task_id: String,
    },

    /// Check failure count for a task
    FailureCount {
        /// Memory log file path
        #[arg(short, long)]
        log_file: String,

        /// Task ID to check
        #[arg(short, long)]
        task_id: String,
    },

    /// Get active failure patterns
    Patterns {
        /// Memory log file path
        #[arg(short, long)]
        log_file: String,
    },

    /// Compile context summary from memory
    Compile {
        /// Memory log file path
        #[arg(short, long)]
        log_file: String,

        /// Optional epic ID to filter by
        #[arg(short, long)]
        epic_id: Option<String>,
    },
}

#[derive(Subcommand)]
enum BeadsStateAction {
    /// Set a state dimension on an issue
    Set {
        /// Issue ID
        issue_id: String,

        /// Dimension to set (mode or health)
        dimension: String,

        /// Value to set
        value: String,

        /// Optional reason for the state change
        #[arg(short, long)]
        reason: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Get state from an issue (specific dimension or all)
    Get {
        /// Issue ID
        issue_id: String,

        /// Dimension to get (mode or health). If omitted, gets all dimensions
        dimension: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "json")]
        format: String,
    },
}

#[derive(Subcommand)]
enum ActivityAction {
    /// List recent activity events
    List {
        /// Issue ID or prefix to filter by (uses --mol flag)
        #[arg(short, long)]
        issue: Option<String>,

        /// Maximum number of events to return
        #[arg(short, long, default_value = "100")]
        limit: usize,

        /// Show events since duration (e.g., "5m", "1h", "30s")
        #[arg(short, long)]
        since: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Stream activity events in real-time (follows)
    Follow {
        /// Issue ID or prefix to filter by
        #[arg(short, long)]
        issue: Option<String>,

        /// Maximum number of events to receive before stopping (0 = unlimited)
        #[arg(short, long, default_value = "0")]
        limit: usize,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum PreflightAction {
    /// Run all preflight checks
    Run {
        /// Optional issue ID to scope the preflight
        #[arg(short, long)]
        issue: Option<String>,

        /// Comma-separated list of specific checks to run (tests,lint,build,uncommitted)
        #[arg(short, long)]
        checks: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Run a single named check
    Check {
        /// Check name: tests, lint, build, uncommitted
        name: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List available checks
    List {
        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum WorktreeAction {
    /// Create a new worktree with beads redirect configuration
    Create {
        /// Name for the worktree (used as directory name)
        name: String,

        /// Branch name for the worktree (defaults to name)
        #[arg(short, long)]
        branch: Option<String>,

        /// Path for the worktree (defaults to ./<name>)
        #[arg(short, long)]
        path: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List all worktrees in the repository
    List {
        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Remove a worktree with safety checks
    Remove {
        /// Name or path of the worktree to remove
        name_or_path: String,

        /// Skip safety checks (uncommitted changes, unpushed commits)
        #[arg(long)]
        force: bool,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Get status of a specific worktree
    Status {
        /// Name or path of the worktree to check
        name_or_path: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show info about the current worktree
    Info {
        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum GateAction {
    /// Create a new gate for async coordination
    Create {
        /// Issue ID to associate the gate with
        issue_id: String,

        /// Gate type: human, timer, gh:run, gh:pr, bead
        #[arg(short = 't', long)]
        gate_type: String,

        /// Timer duration for timer gates (e.g., "1h", "30m")
        #[arg(long)]
        timer: Option<String>,

        /// GitHub check name or run ID for GitHub gates
        #[arg(long)]
        github_check: Option<String>,

        /// Target bead reference for bead gates (format: "rig:bead-id")
        #[arg(long)]
        await_bead: Option<String>,

        /// Optional title for the gate
        #[arg(long)]
        title: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Check the status of a gate
    Check {
        /// Gate ID to check
        gate_id: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Approve/resolve a gate manually (for human approval gates)
    Approve {
        /// Gate ID to approve
        gate_id: String,

        /// Reason for approval
        #[arg(short, long)]
        reason: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List gates with optional filtering
    List {
        /// Issue ID to filter by
        #[arg(short, long)]
        issue: Option<String>,

        /// Include closed gates
        #[arg(short, long)]
        all: bool,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Wait for a gate to be resolved
    Wait {
        /// Gate ID to wait for
        gate_id: String,

        /// Timeout duration (e.g., "5m", "1h")
        #[arg(short, long, default_value = "5m")]
        timeout: String,

        /// Poll interval (e.g., "5s", "30s")
        #[arg(short, long, default_value = "5s")]
        poll_interval: Option<String>,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Evaluate all open gates and auto-close resolved ones
    Evaluate {
        /// Gate type filter: human, timer, gh:run, gh:pr, bead
        #[arg(short = 't', long)]
        gate_type: Option<String>,

        /// Dry run - show what would happen without making changes
        #[arg(long)]
        dry_run: bool,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Get detailed information about a gate
    Show {
        /// Gate ID to show
        gate_id: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Add a waiter to a gate for wake notifications
    AddWaiter {
        /// Gate ID
        gate_id: String,

        /// Waiter address (e.g., "rig/polecats/Name")
        waiter: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum SwarmAction {
    /// Start a new swarm for parallel epic execution
    Start {
        /// Epic ID to orchestrate
        epic_id: String,

        /// Maximum number of workers
        #[arg(short, long, default_value = "4")]
        workers: usize,

        /// Coordinator address (e.g., "gastown/witness")
        #[arg(short, long)]
        coordinator: Option<String>,

        /// Force creation even if swarm already exists
        #[arg(long)]
        force: bool,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Join an existing swarm as a worker
    Join {
        /// Epic ID of the swarm to join
        epic_id: String,

        /// Worker ID for this agent
        #[arg(short, long)]
        worker: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Get status of a swarm
    Status {
        /// Epic ID or swarm molecule ID
        epic_id: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Stop a swarm
    Stop {
        /// Epic ID of the swarm to stop
        epic_id: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Claim the next available task from a swarm
    Claim {
        /// Epic ID of the swarm
        epic_id: String,

        /// Worker ID claiming the task
        #[arg(short, long)]
        worker: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Report task completion
    Complete {
        /// Epic ID of the swarm
        epic_id: String,

        /// Task ID that was completed
        #[arg(short, long)]
        task: String,

        /// Worker ID that completed the task
        #[arg(short, long)]
        worker: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Report task failure
    Failed {
        /// Epic ID of the swarm
        epic_id: String,

        /// Task ID that failed
        #[arg(short, long)]
        task: String,

        /// Worker ID that encountered the failure
        #[arg(short, long)]
        worker: String,

        /// Reason for failure
        #[arg(short, long)]
        reason: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Validate an epic's structure for swarm execution
    Validate {
        /// Epic ID to validate
        epic_id: String,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List all active swarms
    List {
        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum StateAction {
    /// Create new session state
    New {
        /// Session ID
        #[arg(short, long)]
        session_id: String,

        /// Workflow mode
        #[arg(short, long)]
        mode: Option<String>,

        /// Epic ID
        #[arg(short, long)]
        epic_id: Option<String>,

        /// Molecule ID
        #[arg(long)]
        mol_id: Option<String>,

        /// Complexity level
        #[arg(short, long)]
        complexity: Option<String>,

        /// Max iterations
        #[arg(long)]
        max_iterations: Option<u32>,
    },

    /// Load state from JSON
    Load {
        /// JSON string to parse
        json: String,
    },

    /// Update state field
    Update {
        /// Current state as JSON
        #[arg(short, long)]
        state: String,

        /// Field to update (e.g., "iteration_count", "mode")
        #[arg(short, long)]
        field: String,

        /// New value
        #[arg(short, long)]
        value: String,
    },

    /// Check if loop should continue
    ShouldContinue {
        /// Current state as JSON
        #[arg(short, long)]
        state: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::DetectComplexity { task, format } => {
            let complexity = detect_complexity(&task);
            output_result(&format, "complexity", &complexity.to_string());
        }

        Commands::DetectFramework { dir, format } => {
            let directory = dir.unwrap_or_else(|| ".".to_string());
            let (framework, test_cmd) = detect_framework(&directory);
            if format == "json" {
                let result = json!({
                    "framework": framework,
                    "test_command": test_cmd
                });
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            } else {
                println!("framework={}", framework);
                println!("test_command={}", test_cmd);
            }
        }

        Commands::CalcIterations {
            mode,
            complexity,
            format,
        } => {
            let wf_mode = mode
                .parse::<WorkflowMode>()
                .unwrap_or(WorkflowMode::Building);
            let cx = complexity
                .parse::<Complexity>()
                .unwrap_or(Complexity::Standard);
            let iterations = calculate_max_iterations(&wf_mode, &cx);
            output_result(&format, "max_iterations", &iterations.to_string());
        }

        Commands::State { action } => match action {
            StateAction::New {
                session_id,
                mode,
                epic_id,
                mol_id,
                complexity,
                max_iterations,
            } => {
                let wf_mode = mode
                    .and_then(|m| m.parse::<WorkflowMode>().ok())
                    .unwrap_or(WorkflowMode::Building);
                let cx = complexity
                    .and_then(|c| c.parse::<Complexity>().ok())
                    .unwrap_or(Complexity::Standard);

                let state = SessionState::new(session_id)
                    .with_mode(wf_mode)
                    .with_epic_id(epic_id)
                    .with_molecule_id(mol_id)
                    .with_complexity(cx)
                    .with_max_iterations(max_iterations);

                println!("{}", serde_json::to_string_pretty(&state).unwrap());
            }

            StateAction::Load { json } => match serde_json::from_str::<SessionState>(&json) {
                Ok(state) => {
                    println!("{}", serde_json::to_string_pretty(&state).unwrap());
                }
                Err(e) => {
                    eprintln!("Error parsing state: {}", e);
                    std::process::exit(1);
                }
            },

            StateAction::Update {
                state,
                field,
                value,
            } => match serde_json::from_str::<SessionState>(&state) {
                Ok(mut s) => {
                    if let Err(e) = s.update_field(&field, &value) {
                        eprintln!("Error updating field: {}", e);
                        std::process::exit(1);
                    }
                    println!("{}", serde_json::to_string_pretty(&s).unwrap());
                }
                Err(e) => {
                    eprintln!("Error parsing state: {}", e);
                    std::process::exit(1);
                }
            },

            StateAction::ShouldContinue { state } => {
                match serde_json::from_str::<SessionState>(&state) {
                    Ok(s) => {
                        let should_continue = s.should_continue();
                        let result = json!({
                            "should_continue": should_continue,
                            "reason": s.continuation_reason()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    }
                    Err(e) => {
                        eprintln!("Error parsing state: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },

        Commands::Info { format } => {
            let info = json!({
                "version": env!("CARGO_PKG_VERSION"),
                "capabilities": [
                    "detect-complexity",
                    "detect-framework",
                    "calc-iterations",
                    "state-management",
                    "health-check",
                    "security-validation",
                    "procedural-memory"
                ],
                "complexity_levels": ["trivial", "simple", "standard", "critical"],
                "workflow_modes": ["planning", "building", "paused", "complete"],
                "health_statuses": ["healthy", "warning", "degraded", "critical"],
                "risk_levels": ["safe", "low", "medium", "high", "blocked"]
            });
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&info).unwrap());
            } else {
                println!("ralph-beads-cli v{}", env!("CARGO_PKG_VERSION"));
                println!("\nCapabilities:");
                println!("  - detect-complexity: Analyze task description for complexity");
                println!("  - detect-framework: Detect test framework from project files");
                println!("  - calc-iterations: Calculate max iterations for mode/complexity");
                println!("  - state-management: Manage session state");
                println!("  - health-check: Run pre-execution diagnostics");
                println!("  - security-validation: Validate commands against security rules");
                println!("  - procedural-memory: Track failures and workarounds");
            }
        }

        Commands::Health { dir, format } => {
            let directory = dir.unwrap_or_else(|| ".".to_string());
            let checker = HealthChecker::new(&directory);
            let report = checker.check_all();

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("Health Report: {}", report.summary);
                println!("Status: {:?}", report.status);
                println!("Can proceed: {}", report.can_proceed);
                println!("\nChecks:");
                for check in &report.checks {
                    let icon = match check.status {
                        health::HealthStatus::Healthy => "✓",
                        health::HealthStatus::Warning => "⚠",
                        health::HealthStatus::Degraded => "!",
                        health::HealthStatus::Critical => "✗",
                    };
                    println!("  {} {}: {}", icon, check.name, check.message);
                    if let Some(fix) = &check.fix {
                        println!("    Fix: {}", fix);
                    }
                }
            }
        }

        Commands::Validate {
            command,
            project_root,
            format,
        } => {
            let mut validator = SecurityValidator::new();
            if let Some(root) = project_root {
                validator = validator.with_project_root(&root);
            }

            let result = validator.validate(&command);

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            } else {
                let icon = if result.allowed { "✓" } else { "✗" };
                println!("{} Command: {}", icon, command);
                println!("Allowed: {}", result.allowed);
                println!("Risk level: {:?}", result.risk_level);
                println!("Reason: {}", result.reason);
                if let Some(alt) = &result.alternative {
                    println!("Alternative: {}", alt);
                }
            }
        }

        Commands::Memory { action } => match action {
            MemoryAction::Success {
                log_file,
                task_id,
                description,
                context,
            } => {
                let mut memory = ProceduralMemory::new(&log_file);
                let mut entry = memory::MemoryEntry::success(&task_id, &description);

                // Parse context as tags if provided
                if let Some(ctx) = context {
                    if let Ok(tags) = serde_json::from_str::<Vec<String>>(&ctx) {
                        entry = entry.with_tags(tags);
                    }
                }

                if let Err(e) = memory.append(entry) {
                    eprintln!("Error recording success: {}", e);
                    std::process::exit(1);
                }
                println!("{{\"status\": \"recorded\", \"type\": \"success\"}}");
            }

            MemoryAction::Failure {
                log_file,
                task_id,
                error,
                context,
            } => {
                let mut memory = ProceduralMemory::new(&log_file);
                let mut entry = memory::MemoryEntry::failure(&task_id, "Task failed", &error);

                // Parse context as tags if provided
                if let Some(ctx) = context {
                    if let Ok(tags) = serde_json::from_str::<Vec<String>>(&ctx) {
                        entry = entry.with_tags(tags);
                    }
                }

                if let Err(e) = memory.append(entry) {
                    eprintln!("Error recording failure: {}", e);
                    std::process::exit(1);
                }
                println!("{{\"status\": \"recorded\", \"type\": \"failure\"}}");
            }

            MemoryAction::Workaround {
                log_file,
                task_id: _,
                description,
                original_error,
            } => {
                let mut memory = ProceduralMemory::new(&log_file);
                let details =
                    original_error.unwrap_or_else(|| "No original error provided".to_string());
                let entry = memory::MemoryEntry::workaround(&description, &details);

                if let Err(e) = memory.append(entry) {
                    eprintln!("Error recording workaround: {}", e);
                    std::process::exit(1);
                }
                println!("{{\"status\": \"recorded\", \"type\": \"workaround\"}}");
            }

            MemoryAction::GetFailures { log_file, task_id } => {
                let memory = ProceduralMemory::new(&log_file);
                let failures = memory.get_failures(&task_id);
                println!("{}", serde_json::to_string_pretty(&failures).unwrap());
            }

            MemoryAction::FailureCount { log_file, task_id } => {
                let memory = ProceduralMemory::new(&log_file);
                let count = memory.failure_count(&task_id);
                let result = json!({
                    "task_id": task_id,
                    "failure_count": count,
                    "has_failures": count > 0
                });
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            }

            MemoryAction::Patterns { log_file } => {
                let memory = ProceduralMemory::new(&log_file);
                let patterns = memory.get_failure_patterns();
                println!("{}", serde_json::to_string_pretty(&patterns).unwrap());
            }

            MemoryAction::Compile { log_file, epic_id } => {
                let memory = ProceduralMemory::new(&log_file);
                let context = memory.compile_context(epic_id.as_deref());
                println!("{}", context);
            }
        },

        Commands::BeadsState { action } => match action {
            BeadsStateAction::Set {
                issue_id,
                dimension,
                value,
                reason,
                format,
            } => match beads_state::set_state(&issue_id, &dimension, &value, reason.as_deref()) {
                Ok(()) => {
                    let result = json!({
                        "success": true,
                        "issue_id": issue_id,
                        "dimension": dimension,
                        "value": value
                    });
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("Set {}={} on {}", dimension, value, issue_id);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "issue_id": issue_id,
                            "dimension": dimension,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            BeadsStateAction::Get {
                issue_id,
                dimension,
                format,
            } => {
                if let Some(dim) = dimension {
                    // Get specific dimension
                    match beads_state::get_state(&issue_id, &dim) {
                        Ok(Some(value)) => {
                            if format == "json" {
                                let result = json!({
                                    "success": true,
                                    "issue_id": issue_id,
                                    "dimension": dim,
                                    "value": value
                                });
                                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                            } else {
                                println!("{}={}", dim, value);
                            }
                        }
                        Ok(None) => {
                            if format == "json" {
                                let result = json!({
                                    "success": true,
                                    "issue_id": issue_id,
                                    "dimension": dim,
                                    "value": null
                                });
                                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                            } else {
                                println!("{} is not set", dim);
                            }
                        }
                        Err(e) => {
                            if format == "json" {
                                let result = json!({
                                    "success": false,
                                    "issue_id": issue_id,
                                    "dimension": dim,
                                    "error": e.to_string()
                                });
                                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                            } else {
                                eprintln!("Error: {}", e);
                            }
                            std::process::exit(1);
                        }
                    }
                } else {
                    // Get all dimensions
                    match beads_state::get_all_state(&issue_id) {
                        Ok(state) => {
                            if format == "json" {
                                let result = json!({
                                    "success": true,
                                    "issue_id": issue_id,
                                    "state": state
                                });
                                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                            } else if state.is_empty() {
                                println!("No state dimensions set on {}", issue_id);
                            } else {
                                println!("State for {}:", issue_id);
                                for (dim, val) in &state {
                                    println!("  {}={}", dim, val);
                                }
                            }
                        }
                        Err(e) => {
                            if format == "json" {
                                let result = json!({
                                    "success": false,
                                    "issue_id": issue_id,
                                    "error": e.to_string()
                                });
                                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                            } else {
                                eprintln!("Error: {}", e);
                            }
                            std::process::exit(1);
                        }
                    }
                }
            }
        },

        Commands::Activity { action } => match action {
            ActivityAction::List {
                issue,
                limit,
                since,
                format,
            } => {
                let events = if let Some(since_str) = since {
                    match get_activity_since(&since_str, issue.as_deref()) {
                        Ok(e) => e,
                        Err(e) => {
                            eprintln!("Error getting activity: {}", e);
                            std::process::exit(1);
                        }
                    }
                } else if let Some(issue_id) = &issue {
                    match get_activity_for_issue(issue_id, limit) {
                        Ok(e) => e,
                        Err(e) => {
                            eprintln!("Error getting activity: {}", e);
                            std::process::exit(1);
                        }
                    }
                } else {
                    match get_recent_activity(limit) {
                        Ok(e) => e,
                        Err(e) => {
                            eprintln!("Error getting activity: {}", e);
                            std::process::exit(1);
                        }
                    }
                };

                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&events).unwrap());
                } else {
                    print!("{}", format_activity_text(&events));
                }
            }

            ActivityAction::Follow {
                issue,
                limit,
                format,
            } => {
                if let Err(e) = stream_activity(issue.as_deref(), limit, &format) {
                    eprintln!("Error streaming activity: {}", e);
                    std::process::exit(1);
                }
            }
        },

        Commands::Preflight { action } => match action {
            PreflightAction::Run {
                issue,
                checks,
                format,
            } => {
                // If specific checks requested, run only those
                if let Some(check_list) = checks {
                    let check_names: Vec<&str> = check_list.split(',').map(|s| s.trim()).collect();
                    let mut results = Vec::new();

                    for check_name in check_names {
                        match preflight::run_single_check(check_name) {
                            Ok(check) => results.push(check),
                            Err(e) => {
                                eprintln!("Error running check '{}': {}", check_name, e);
                                std::process::exit(1);
                            }
                        }
                    }

                    let report = preflight::PreflightReport::from_checks(results);
                    output_preflight_report(&report, &format);
                } else {
                    // Run all checks via bd preflight
                    match preflight::run_preflight(issue.as_deref()) {
                        Ok(report) => {
                            output_preflight_report(&report, &format);
                            if !report.passed {
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error running preflight: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }

            PreflightAction::Check { name, format } => match preflight::run_single_check(&name) {
                Ok(check) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&check).unwrap());
                    } else {
                        let icon = match check.status {
                            preflight::CheckStatus::Passed => "[PASS]",
                            preflight::CheckStatus::Failed => "[FAIL]",
                            preflight::CheckStatus::Skipped => "[SKIP]",
                            preflight::CheckStatus::Warning => "[WARN]",
                        };
                        println!("{} {}", icon, check.name);
                        if let Some(msg) = &check.message {
                            println!("  {}", msg);
                        }
                        if let Some(cmd) = &check.command {
                            println!("  Command: {}", cmd);
                        }
                    }

                    if matches!(check.status, preflight::CheckStatus::Failed) {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Error running check '{}': {}", name, e);
                    std::process::exit(1);
                }
            },

            PreflightAction::List { format } => {
                let checks = preflight::available_checks();
                if format == "json" {
                    let result = json!({
                        "available_checks": checks
                    });
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                } else {
                    println!("Available preflight checks:");
                    for check in checks {
                        println!("  - {}", check);
                    }
                }
            }
        },

        Commands::Worktree { action } => match action {
            WorktreeAction::Create {
                name,
                branch,
                path,
                format,
            } => {
                let branch_name = branch.as_deref().unwrap_or(&name);
                let worktree_path = path.unwrap_or_else(|| format!("./{}", name));
                match worktree::create_worktree(&name, branch_name, &worktree_path) {
                    Ok(info) => {
                        if format == "json" {
                            println!("{}", serde_json::to_string_pretty(&info).unwrap());
                        } else {
                            println!("Created worktree: {}", info.path);
                            println!("Branch: {}", info.branch);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error creating worktree: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            WorktreeAction::List { format } => match worktree::list_worktrees() {
                Ok(worktrees) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&worktrees).unwrap());
                    } else {
                        for wt in &worktrees {
                            println!(
                                "{} -> {} ({})",
                                wt.path,
                                wt.branch,
                                if wt.is_main { "main" } else { "worktree" }
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error listing worktrees: {}", e);
                    std::process::exit(1);
                }
            },
            WorktreeAction::Remove {
                name_or_path,
                force,
                format,
            } => match worktree::remove_worktree(&name_or_path, force) {
                Ok(()) => {
                    if format == "json" {
                        println!("{{\"success\": true, \"removed\": \"{}\"}}", name_or_path);
                    } else {
                        println!("Removed worktree: {}", name_or_path);
                    }
                }
                Err(e) => {
                    eprintln!("Error removing worktree: {}", e);
                    std::process::exit(1);
                }
            },
            WorktreeAction::Status { name_or_path, format } => {
                match worktree::get_worktree_status(&name_or_path) {
                    Ok(status) => {
                        if format == "json" {
                            println!("{}", serde_json::to_string_pretty(&status).unwrap());
                        } else {
                            println!("Worktree: {}", name_or_path);
                            println!("Path: {}", status.path);
                            println!("Branch: {}", status.branch);
                            println!("Clean: {}", status.is_clean);
                            if status.is_detached {
                                println!("HEAD is detached");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error getting worktree status: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            WorktreeAction::Info { format } => match worktree::get_current_worktree_info() {
                Ok(info) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&info).unwrap());
                    } else {
                        if info.is_worktree {
                            println!(
                                "Path: {}",
                                info.path.as_deref().unwrap_or("unknown")
                            );
                            println!("Name: {}", info.name.as_deref().unwrap_or("unknown"));
                            println!("Branch: {}", info.branch.as_deref().unwrap_or("unknown"));
                            println!(
                                "Main repo: {}",
                                info.main_repo.as_deref().unwrap_or("unknown")
                            );
                        } else {
                            println!("Not in a worktree (main repository)");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error getting worktree info: {}", e);
                    std::process::exit(1);
                }
            },
        },

        Commands::Gate { action } => match action {
            GateAction::Create {
                issue_id,
                gate_type,
                timer,
                github_check,
                await_bead,
                title,
                format,
            } => {
                let gt = match gate_type.parse::<gates::GateType>() {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                };

                let mut config = gates::GateConfig::new().with_issue(&issue_id);

                if let Some(t) = title {
                    config = config.with_title(&t);
                }
                if let Some(t) = timer {
                    config = config.with_timer(&t);
                }
                if let Some(c) = github_check {
                    config = config.with_github_check(&c);
                }
                if let Some(b) = await_bead {
                    config = config.with_await_bead(&b);
                }

                match gates::create_gate(gt, config) {
                    Ok(gate_id) => {
                        if format == "json" {
                            let result = json!({
                                "success": true,
                                "gate_id": gate_id,
                                "issue_id": issue_id,
                                "gate_type": gate_type
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else {
                            println!("Created gate: {}", gate_id);
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let result = json!({
                                "success": false,
                                "error": e.to_string()
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else {
                            eprintln!("Error creating gate: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }

            GateAction::Check { gate_id, format } => match gates::check_gate(&gate_id) {
                Ok(status) => {
                    if format == "json" {
                        let result = json!({
                            "gate_id": gate_id,
                            "status": status.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("{}: {}", gate_id, status);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "gate_id": gate_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error checking gate: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            GateAction::Approve {
                gate_id,
                reason,
                format,
            } => match gates::approve_gate(&gate_id, reason.as_deref()) {
                Ok(()) => {
                    if format == "json" {
                        let result = json!({
                            "success": true,
                            "gate_id": gate_id,
                            "action": "approved"
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("Approved gate: {}", gate_id);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "gate_id": gate_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error approving gate: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            GateAction::List { issue, all, format } => {
                match gates::list_gates(issue.as_deref(), all) {
                    Ok(gates_list) => {
                        if format == "json" {
                            println!("{}", serde_json::to_string_pretty(&gates_list).unwrap());
                        } else if gates_list.is_empty() {
                            println!("No gates found");
                        } else {
                            for gate in &gates_list {
                                let title = gate.title.as_deref().unwrap_or("-");
                                println!(
                                    "{} [{}] {} - {}",
                                    gate.id, gate.gate_type, gate.status, title
                                );
                            }
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let result = json!({
                                "error": e.to_string()
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else {
                            eprintln!("Error listing gates: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }

            GateAction::Wait {
                gate_id,
                timeout,
                poll_interval,
                format,
            } => {
                let timeout_duration = match gates::parse_duration(&timeout) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Invalid timeout: {}", e);
                        std::process::exit(1);
                    }
                };

                let poll_duration = poll_interval.and_then(|p| gates::parse_duration(&p).ok());

                match gates::wait_for_gate(&gate_id, timeout_duration, poll_duration) {
                    Ok(status) => {
                        if format == "json" {
                            let result = json!({
                                "gate_id": gate_id,
                                "status": status.to_string(),
                                "resolved": status != gates::GateStatus::Pending
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else {
                            println!("{}: {}", gate_id, status);
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let result = json!({
                                "gate_id": gate_id,
                                "error": e.to_string()
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else {
                            eprintln!("Error waiting for gate: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }

            GateAction::Evaluate {
                gate_type,
                dry_run,
                format,
            } => {
                let gt = gate_type.and_then(|t| t.parse::<gates::GateType>().ok());

                match gates::evaluate_gates(gt, dry_run) {
                    Ok(results) => {
                        if format == "json" {
                            println!("{}", serde_json::to_string_pretty(&results).unwrap());
                        } else if results.is_empty() {
                            println!("No gates evaluated");
                        } else {
                            for result in &results {
                                let action = if result.resolved {
                                    "resolved"
                                } else {
                                    "unchanged"
                                };
                                println!(
                                    "{}: {} -> {} ({})",
                                    result.gate_id,
                                    result.previous_status,
                                    result.new_status,
                                    action
                                );
                            }
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let result = json!({
                                "error": e.to_string()
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else {
                            eprintln!("Error evaluating gates: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }

            GateAction::Show { gate_id, format } => match gates::get_gate(&gate_id) {
                Ok(gate) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&gate).unwrap());
                    } else {
                        println!("ID: {}", gate.id);
                        println!("Type: {}", gate.gate_type);
                        println!("Status: {}", gate.status);
                        if let Some(title) = &gate.title {
                            println!("Title: {}", title);
                        }
                        if let Some(issue_id) = &gate.issue_id {
                            println!("Issue: {}", issue_id);
                        }
                        if let Some(await_id) = &gate.await_id {
                            println!("Await ID: {}", await_id);
                        }
                        if let Some(created_at) = &gate.created_at {
                            println!("Created: {}", created_at);
                        }
                        if !gate.waiters.is_empty() {
                            println!("Waiters: {}", gate.waiters.join(", "));
                        }
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "gate_id": gate_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error getting gate: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            GateAction::AddWaiter {
                gate_id,
                waiter,
                format,
            } => match gates::add_waiter(&gate_id, &waiter) {
                Ok(()) => {
                    if format == "json" {
                        let result = json!({
                            "success": true,
                            "gate_id": gate_id,
                            "waiter": waiter
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("Added waiter {} to gate {}", waiter, gate_id);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "gate_id": gate_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error adding waiter: {}", e);
                    }
                    std::process::exit(1);
                }
            },
        },

        Commands::Swarm { action } => match action {
            SwarmAction::Start {
                epic_id,
                workers,
                coordinator,
                force,
                format,
            } => {
                let mut config = swarm::SwarmConfig::new(&epic_id)
                    .with_max_workers(workers)
                    .with_force(force);

                if let Some(coord) = coordinator {
                    config = config.with_coordinator(&coord);
                }

                match swarm::start_swarm(config) {
                    Ok(state) => {
                        if format == "json" {
                            println!("{}", serde_json::to_string_pretty(&state).unwrap());
                        } else {
                            println!("Started swarm for epic: {}", epic_id);
                            println!(
                                "Tasks: {} total, {} ready",
                                state.tasks_total, state.tasks_ready
                            );
                            if let Some(swarm_id) = &state.swarm_id {
                                println!("Swarm ID: {}", swarm_id);
                            }
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let result = json!({
                                "success": false,
                                "epic_id": epic_id,
                                "error": e.to_string()
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else {
                            eprintln!("Error starting swarm: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }

            SwarmAction::Join {
                epic_id,
                worker,
                format,
            } => match swarm::join_swarm(&epic_id, &worker) {
                Ok(()) => {
                    if format == "json" {
                        let result = json!({
                            "success": true,
                            "epic_id": epic_id,
                            "worker_id": worker,
                            "action": "joined"
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("Joined swarm {} as worker {}", epic_id, worker);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "epic_id": epic_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error joining swarm: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            SwarmAction::Status { epic_id, format } => match swarm::get_swarm_status(&epic_id) {
                Ok(state) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&state).unwrap());
                    } else {
                        println!("Swarm Status: {}", state.status);
                        println!("Epic: {}", state.epic_id);
                        println!(
                            "Progress: {:.1}% ({}/{} tasks)",
                            state.progress_percent, state.tasks_completed, state.tasks_total
                        );
                        println!(
                            "  In Progress: {}, Ready: {}, Blocked: {}",
                            state.tasks_in_progress, state.tasks_ready, state.tasks_blocked
                        );
                        if !state.active_workers.is_empty() {
                            println!("Active Workers: {}", state.active_workers.join(", "));
                        }
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "epic_id": epic_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error getting swarm status: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            SwarmAction::Stop { epic_id, format } => match swarm::stop_swarm(&epic_id) {
                Ok(()) => {
                    if format == "json" {
                        let result = json!({
                            "success": true,
                            "epic_id": epic_id,
                            "action": "stopped"
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("Stopped swarm: {}", epic_id);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "epic_id": epic_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error stopping swarm: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            SwarmAction::Claim {
                epic_id,
                worker,
                format,
            } => match swarm::claim_next_task(&epic_id, &worker) {
                Ok(Some(task_id)) => {
                    if format == "json" {
                        let result = json!({
                            "success": true,
                            "epic_id": epic_id,
                            "worker_id": worker,
                            "task_id": task_id
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("Claimed task: {}", task_id);
                    }
                }
                Ok(None) => {
                    if format == "json" {
                        let result = json!({
                            "success": true,
                            "epic_id": epic_id,
                            "worker_id": worker,
                            "task_id": null,
                            "message": "No tasks available"
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("No tasks available to claim");
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "epic_id": epic_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error claiming task: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            SwarmAction::Complete {
                epic_id,
                task,
                worker,
                format,
            } => match swarm::report_task_complete(&epic_id, &task, &worker) {
                Ok(()) => {
                    if format == "json" {
                        let result = json!({
                            "success": true,
                            "epic_id": epic_id,
                            "task_id": task,
                            "worker_id": worker,
                            "action": "completed"
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("Completed task: {}", task);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "task_id": task,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error reporting completion: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            SwarmAction::Failed {
                epic_id,
                task,
                worker,
                reason,
                format,
            } => match swarm::report_task_failed(&epic_id, &task, &worker, &reason) {
                Ok(()) => {
                    if format == "json" {
                        let result = json!({
                            "success": true,
                            "epic_id": epic_id,
                            "task_id": task,
                            "worker_id": worker,
                            "action": "failed",
                            "reason": reason
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        println!("Reported failure for task: {}", task);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "task_id": task,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error reporting failure: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            SwarmAction::Validate { epic_id, format } => match swarm::validate_epic(&epic_id) {
                Ok(validation) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&validation).unwrap());
                    } else {
                        let status = if validation.is_valid { "VALID" } else { "INVALID" };
                        println!("Epic {}: {}", epic_id, status);
                        println!("  Ready fronts: {}", validation.ready_fronts);
                        println!("  Estimated sessions: {}", validation.estimated_sessions);
                        println!("  Max parallelism: {}", validation.max_parallelism);

                        if !validation.warnings.is_empty() {
                            println!("\nWarnings:");
                            for warning in &validation.warnings {
                                println!("  - {}", warning);
                            }
                        }

                        if !validation.errors.is_empty() {
                            println!("\nErrors:");
                            for error in &validation.errors {
                                println!("  - {}", error);
                            }
                        }
                    }

                    if !validation.is_valid {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "epic_id": epic_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error validating epic: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            SwarmAction::List { format } => match swarm::list_swarms() {
                Ok(swarms) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&swarms).unwrap());
                    } else if swarms.is_empty() {
                        println!("No active swarms");
                    } else {
                        for s in &swarms {
                            println!(
                                "{}: {} - {:.1}% ({}/{})",
                                s.epic_id,
                                s.status,
                                s.progress_percent,
                                s.tasks_completed,
                                s.tasks_total
                            );
                        }
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error listing swarms: {}", e);
                    }
                    std::process::exit(1);
                }
            },
        },

        Commands::Lint { action } => match action {
            LintAction::Issue { issue_id, format } => match lint::lint_issue(&issue_id) {
                Ok(results) => {
                    let report = lint::LintReport::from_results(results);
                    output_lint_report(&report, &format);
                    if !report.passed {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "issue_id": issue_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error linting issue: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            LintAction::Epic { epic_id, format } => match lint::lint_epic(&epic_id) {
                Ok(report) => {
                    output_lint_report(&report, &format);
                    if !report.passed {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "epic_id": epic_id,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error linting epic: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            LintAction::All { severity, format } => match lint::lint_all() {
                Ok(mut report) => {
                    // Filter by severity if specified
                    if severity != "all" {
                        let min_severity = match severity.to_lowercase().as_str() {
                            "error" => lint::LintSeverity::Error,
                            "warning" => lint::LintSeverity::Warning,
                            _ => lint::LintSeverity::Info,
                        };
                        report.results = lint::filter_by_severity(report.results, min_severity);
                        // Recalculate counts
                        report = lint::LintReport::from_results(report.results);
                    }

                    output_lint_report(&report, &format);
                    if !report.passed {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let result = json!({
                            "success": false,
                            "error": e.to_string()
                        });
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        eprintln!("Error linting all issues: {}", e);
                    }
                    std::process::exit(1);
                }
            },

            LintAction::CheckAc { issue_id, format } => {
                match lint::check_acceptance_criteria(&issue_id) {
                    Ok(has_ac) => {
                        if format == "json" {
                            let result = json!({
                                "success": true,
                                "issue_id": issue_id,
                                "has_acceptance_criteria": has_ac
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else if has_ac {
                            println!("[PASS] Issue {} has acceptance criteria", issue_id);
                        } else {
                            println!("[FAIL] Issue {} is missing acceptance criteria", issue_id);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let result = json!({
                                "success": false,
                                "issue_id": issue_id,
                                "error": e.to_string()
                            });
                            println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        } else {
                            eprintln!("Error checking acceptance criteria: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }
        },
    }
}

fn output_lint_report(report: &lint::LintReport, format: &str) {
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(report).unwrap());
    } else {
        let overall_icon = if report.passed { "[PASS]" } else { "[FAIL]" };
        println!("{} Lint: {}", overall_icon, report.summary);
        println!();

        if report.results.is_empty() {
            println!("  No issues found");
        } else {
            for result in &report.results {
                let icon = match result.severity {
                    lint::LintSeverity::Error => "[ERROR]",
                    lint::LintSeverity::Warning => "[WARN]",
                    lint::LintSeverity::Info => "[INFO]",
                };
                println!("  {} {} - {}", icon, result.issue_id, result.message);
                if let Some(suggestion) = &result.suggestion {
                    // Truncate long suggestions
                    let truncated = if suggestion.len() > 100 {
                        format!("{}...", &suggestion[..100])
                    } else {
                        suggestion.clone()
                    };
                    println!("      Suggestion: {}", truncated);
                }
            }
        }
    }
}

fn output_preflight_report(report: &preflight::PreflightReport, format: &str) {
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(report).unwrap());
    } else {
        let overall_icon = if report.passed { "[PASS]" } else { "[FAIL]" };
        println!("{} Preflight: {}", overall_icon, report.summary);
        println!();

        for check in &report.checks {
            let icon = match check.status {
                preflight::CheckStatus::Passed => "[PASS]",
                preflight::CheckStatus::Failed => "[FAIL]",
                preflight::CheckStatus::Skipped => "[SKIP]",
                preflight::CheckStatus::Warning => "[WARN]",
            };
            println!("  {} {}", icon, check.name);
            if let Some(msg) = &check.message {
                // Truncate long messages
                let truncated = if msg.len() > 100 {
                    format!("{}...", &msg[..100])
                } else {
                    msg.clone()
                };
                println!("      {}", truncated);
            }
        }
    }
}

fn output_result(format: &str, key: &str, value: &str) {
    if format == "json" {
        println!("{}", json!({ key: value }));
    } else {
        println!("{}={}", key, value);
    }
}
