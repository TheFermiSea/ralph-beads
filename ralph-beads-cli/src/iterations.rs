use crate::complexity::Complexity;
use crate::state::WorkflowMode;

/// Default iterations for planning mode by complexity
const PLANNING_ITERATIONS: [(Complexity, u32); 4] = [
    (Complexity::Trivial, 2),
    (Complexity::Simple, 3),
    (Complexity::Standard, 5),
    (Complexity::Critical, 8),
];

/// Default iterations for building mode by complexity
const BUILDING_ITERATIONS: [(Complexity, u32); 4] = [
    (Complexity::Trivial, 5),
    (Complexity::Simple, 10),
    (Complexity::Standard, 20),
    (Complexity::Critical, 40),
];

/// Calculate the maximum number of iterations based on workflow mode and complexity
///
/// # Arguments
/// * `mode` - The current workflow mode (planning or building)
/// * `complexity` - The detected or specified complexity level
///
/// # Returns
/// The recommended maximum number of iterations
///
/// # Iteration Scaling Table
///
/// | Complexity | Planning | Building | Validation     |
/// |------------|----------|----------|----------------|
/// | Trivial    | 2        | 5        | Skip           |
/// | Simple     | 3        | 10       | Skip           |
/// | Standard   | 5        | 20       | Auto-enable    |
/// | Critical   | 8        | 40       | Required       |
pub fn calculate_max_iterations(mode: &WorkflowMode, complexity: &Complexity) -> u32 {
    match mode {
        WorkflowMode::Planning => {
            for (cx, iter) in &PLANNING_ITERATIONS {
                if cx == complexity {
                    return *iter;
                }
            }
            5 // default for planning
        }
        WorkflowMode::Building => {
            for (cx, iter) in &BUILDING_ITERATIONS {
                if cx == complexity {
                    return *iter;
                }
            }
            20 // default for building
        }
        // Paused and Complete don't need iteration calculations
        WorkflowMode::Paused | WorkflowMode::Complete => 0,
    }
}

/// Get iteration limits for a complexity level
///
/// Returns (planning_iterations, building_iterations)
pub fn get_iteration_limits(complexity: &Complexity) -> (u32, u32) {
    let planning = PLANNING_ITERATIONS
        .iter()
        .find(|(cx, _)| cx == complexity)
        .map(|(_, iter)| *iter)
        .unwrap_or(5);

    let building = BUILDING_ITERATIONS
        .iter()
        .find(|(cx, _)| cx == complexity)
        .map(|(_, iter)| *iter)
        .unwrap_or(20);

    (planning, building)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planning_iterations() {
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Planning, &Complexity::Trivial),
            2
        );
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Planning, &Complexity::Simple),
            3
        );
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Planning, &Complexity::Standard),
            5
        );
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Planning, &Complexity::Critical),
            8
        );
    }

    #[test]
    fn test_building_iterations() {
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Building, &Complexity::Trivial),
            5
        );
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Building, &Complexity::Simple),
            10
        );
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Building, &Complexity::Standard),
            20
        );
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Building, &Complexity::Critical),
            40
        );
    }

    #[test]
    fn test_paused_and_complete_return_zero() {
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Paused, &Complexity::Standard),
            0
        );
        assert_eq!(
            calculate_max_iterations(&WorkflowMode::Complete, &Complexity::Standard),
            0
        );
    }

    #[test]
    fn test_get_iteration_limits() {
        assert_eq!(get_iteration_limits(&Complexity::Trivial), (2, 5));
        assert_eq!(get_iteration_limits(&Complexity::Simple), (3, 10));
        assert_eq!(get_iteration_limits(&Complexity::Standard), (5, 20));
        assert_eq!(get_iteration_limits(&Complexity::Critical), (8, 40));
    }
}
