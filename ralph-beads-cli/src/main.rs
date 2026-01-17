mod activity;
mod beads_state;
mod complexity;
mod framework;
mod health;
mod iterations;
mod memory;
mod security;
mod state;

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
    }
}

fn output_result(format: &str, key: &str, value: &str) {
    if format == "json" {
        println!("{}", json!({ key: value }));
    } else {
        println!("{}={}", key, value);
    }
}
