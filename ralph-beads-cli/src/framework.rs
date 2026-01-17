use std::fs;
use std::path::Path;

/// Detected framework information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkInfo {
    pub framework: String,
    pub test_command: String,
}

/// Detect test framework from project directory
///
/// Examines project files to determine the framework and appropriate test command.
/// Order of detection:
/// 1. Rust (Cargo.toml) - prefers cargo-nextest if available
/// 2. Python (pyproject.toml, setup.py) - prefers pytest
/// 3. Node.js (package.json) - uses npm test
///
/// # Returns
/// Tuple of (framework_name, test_command)
pub fn detect_framework(dir: &str) -> (String, String) {
    let path = Path::new(dir);

    // Check for Rust project
    if path.join("Cargo.toml").exists() {
        let test_cmd = if has_cargo_nextest() {
            "cargo nextest run"
        } else {
            "cargo test"
        };
        return ("rust".to_string(), test_cmd.to_string());
    }

    // Check for Python project
    if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
        let test_cmd = if path.join("pytest.ini").exists()
            || path.join("pyproject.toml").exists()
            || has_pytest_installed()
        {
            "pytest"
        } else {
            "python -m unittest discover"
        };
        return ("python".to_string(), test_cmd.to_string());
    }

    // Check for Node.js project
    if path.join("package.json").exists() {
        let test_cmd = if has_npm_test_script(path) {
            "npm test"
        } else {
            "echo 'No test script defined'"
        };
        return ("node".to_string(), test_cmd.to_string());
    }

    // Check for Go project
    if path.join("go.mod").exists() {
        return ("go".to_string(), "go test ./...".to_string());
    }

    // Check for Java/Gradle project
    if path.join("build.gradle").exists() || path.join("build.gradle.kts").exists() {
        return ("java".to_string(), "./gradlew test".to_string());
    }

    // Check for Java/Maven project
    if path.join("pom.xml").exists() {
        return ("java".to_string(), "mvn test".to_string());
    }

    // No framework detected
    (
        "none".to_string(),
        "echo 'No test framework detected'".to_string(),
    )
}

/// Check if cargo-nextest is available
fn has_cargo_nextest() -> bool {
    std::process::Command::new("cargo")
        .args(["nextest", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if pytest is installed
fn has_pytest_installed() -> bool {
    std::process::Command::new("pytest")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if package.json has a test script
fn has_npm_test_script(dir: &Path) -> bool {
    let package_json = dir.join("package.json");
    if let Ok(content) = fs::read_to_string(package_json) {
        // Simple check for "test" in scripts
        content.contains(r#""test""#) && content.contains(r#""scripts""#)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn create_temp_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    #[test]
    fn test_detect_rust_project() {
        let dir = create_temp_dir();
        File::create(dir.path().join("Cargo.toml")).unwrap();

        let (framework, test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "rust");
        // Command depends on whether cargo-nextest is installed
        assert!(test_cmd.contains("cargo"));
    }

    #[test]
    fn test_detect_python_project_pyproject() {
        let dir = create_temp_dir();
        File::create(dir.path().join("pyproject.toml")).unwrap();

        let (framework, _test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "python");
    }

    #[test]
    fn test_detect_python_project_setup_py() {
        let dir = create_temp_dir();
        File::create(dir.path().join("setup.py")).unwrap();

        let (framework, _test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "python");
    }

    #[test]
    fn test_detect_node_project_with_test_script() {
        let dir = create_temp_dir();
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"test": "jest"}}"#,
        )
        .unwrap();

        let (framework, test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "node");
        assert_eq!(test_cmd, "npm test");
    }

    #[test]
    fn test_detect_node_project_without_test_script() {
        let dir = create_temp_dir();
        fs::write(dir.path().join("package.json"), r#"{"scripts": {}}"#).unwrap();

        let (framework, test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "node");
        assert!(test_cmd.contains("No test script"));
    }

    #[test]
    fn test_detect_go_project() {
        let dir = create_temp_dir();
        File::create(dir.path().join("go.mod")).unwrap();

        let (framework, test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "go");
        assert_eq!(test_cmd, "go test ./...");
    }

    #[test]
    fn test_detect_gradle_project() {
        let dir = create_temp_dir();
        File::create(dir.path().join("build.gradle")).unwrap();

        let (framework, test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "java");
        assert_eq!(test_cmd, "./gradlew test");
    }

    #[test]
    fn test_detect_maven_project() {
        let dir = create_temp_dir();
        File::create(dir.path().join("pom.xml")).unwrap();

        let (framework, test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "java");
        assert_eq!(test_cmd, "mvn test");
    }

    #[test]
    fn test_detect_no_framework() {
        let dir = create_temp_dir();

        let (framework, _test_cmd) = detect_framework(dir.path().to_str().unwrap());

        assert_eq!(framework, "none");
    }
}
