//! Ralph-Beads CLI
//!
//! Rust CLI helper for the ralph-beads plugin providing:
//! - Complexity detection from task descriptions
//! - Test framework detection
//! - Iteration calculation based on mode and complexity

mod complexity;
mod framework;
mod state;

use clap::{Parser, Subcommand};
use serde_json::json;

use complexity::{calculate_max_iterations, detect_complexity, Complexity};
use framework::detect_framework;
use state::WorkflowMode;

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

    /// Output information about CLI capabilities
    Info {
        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

/// Helper function to output a key-value result in the specified format
fn output_result(format: &str, key: &str, value: &str) {
    if format == "json" {
        println!("{}", json!({ key: value }));
    } else {
        println!("{}={}", key, value);
    }
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

        Commands::Info { format } => {
            let info = json!({
                "version": env!("CARGO_PKG_VERSION"),
                "capabilities": [
                    "detect-complexity",
                    "detect-framework",
                    "calc-iterations"
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
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_result_text() {
        // Just verify it doesn't panic
        output_result("text", "key", "value");
    }

    #[test]
    fn test_output_result_json() {
        // Just verify it doesn't panic
        output_result("json", "key", "value");
    }
}
