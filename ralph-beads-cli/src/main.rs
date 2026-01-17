mod complexity;
mod framework;
mod iterations;
mod state;

use clap::{Parser, Subcommand};
use serde_json::json;

use complexity::{detect_complexity, Complexity};
use framework::detect_framework;
use iterations::calculate_max_iterations;
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
                    "state-management"
                ],
                "complexity_levels": ["trivial", "simple", "standard", "critical"],
                "workflow_modes": ["planning", "building", "paused", "complete"]
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
