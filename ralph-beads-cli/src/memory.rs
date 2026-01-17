//! Procedural Memory System
//!
//! Implements an append-only log of what succeeded and failed,
//! inspired by Context-Engine's four-layer memory architecture.
//!
//! This module provides:
//! - Failure pattern detection to avoid repeating mistakes
//! - Success pattern recognition for reinforcement
//! - Semantic tagging for searchable failure history
//! - Concurrency-safe file access with exclusive locking

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::Path;

/// Types of memory entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// Task completed successfully
    Success,
    /// Task failed (with reason)
    Failure,
    /// Workaround discovered
    Workaround,
    /// Pattern learned
    Pattern,
    /// Decision made (for context)
    Decision,
}

/// A single memory entry in the procedural log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique entry ID
    pub id: String,
    /// Type of memory
    pub memory_type: MemoryType,
    /// Associated task ID (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Associated epic ID (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_id: Option<String>,
    /// Semantic tags for searchability
    #[serde(default)]
    pub tags: Vec<String>,
    /// Short summary
    pub summary: String,
    /// Detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Error message (for failures)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Related file paths
    #[serde(default)]
    pub files: Vec<String>,
    /// Timestamp (ISO 8601)
    pub timestamp: String,
}

impl MemoryEntry {
    /// Create a new success entry
    pub fn success(task_id: &str, summary: &str) -> Self {
        Self {
            id: generate_id(),
            memory_type: MemoryType::Success,
            task_id: Some(task_id.to_string()),
            epic_id: None,
            tags: Vec::new(),
            summary: summary.to_string(),
            details: None,
            error: None,
            files: Vec::new(),
            timestamp: current_timestamp(),
        }
    }

    /// Create a new failure entry
    pub fn failure(task_id: &str, summary: &str, error: &str) -> Self {
        Self {
            id: generate_id(),
            memory_type: MemoryType::Failure,
            task_id: Some(task_id.to_string()),
            epic_id: None,
            tags: Vec::new(),
            summary: summary.to_string(),
            details: None,
            error: Some(error.to_string()),
            files: Vec::new(),
            timestamp: current_timestamp(),
        }
    }

    /// Create a workaround entry
    pub fn workaround(summary: &str, details: &str) -> Self {
        Self {
            id: generate_id(),
            memory_type: MemoryType::Workaround,
            task_id: None,
            epic_id: None,
            tags: Vec::new(),
            summary: summary.to_string(),
            details: Some(details.to_string()),
            error: None,
            files: Vec::new(),
            timestamp: current_timestamp(),
        }
    }

    /// Add tags for searchability
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add related files
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files = files;
        self
    }

    /// Set epic ID
    pub fn with_epic(mut self, epic_id: &str) -> Self {
        self.epic_id = Some(epic_id.to_string());
        self
    }
}

/// Failure pattern for circuit breaker enhancement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// Pattern identifier
    pub pattern: String,
    /// Number of occurrences
    pub count: u32,
    /// Tasks that hit this pattern
    pub affected_tasks: Vec<String>,
    /// Suggested action
    pub suggestion: String,
}

/// Procedural memory store
#[derive(Debug)]
pub struct ProceduralMemory {
    /// Path to the JSONL log file
    log_path: String,
    /// In-memory cache of recent entries
    recent: Vec<MemoryEntry>,
    /// Failure pattern cache
    failure_patterns: HashMap<String, FailurePattern>,
}

impl ProceduralMemory {
    /// Create or load procedural memory from a path
    pub fn new(path: &str) -> Self {
        let mut memory = Self {
            log_path: path.to_string(),
            recent: Vec::new(),
            failure_patterns: HashMap::new(),
        };
        memory.load_recent(100); // Load last 100 entries
        memory.compute_failure_patterns();
        memory
    }

    /// Append an entry to the log (append-only, concurrency-safe)
    ///
    /// Uses exclusive file locking to prevent corruption when multiple
    /// agents (e.g., in Swarm mode) write simultaneously.
    pub fn append(&mut self, entry: MemoryEntry) -> std::io::Result<()> {
        // Ensure directory exists
        if let Some(parent) = Path::new(&self.log_path).parent() {
            fs::create_dir_all(parent)?;
        }

        // Open file for appending
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        // Acquire exclusive lock - blocks until available
        // This prevents data corruption when multiple processes write
        file.lock_exclusive()?;

        // Write the entry (file is locked)
        let mut writer = std::io::BufWriter::new(&file);
        let json = serde_json::to_string(&entry).unwrap();
        writeln!(writer, "{}", json)?;
        writer.flush()?;

        // Release lock (explicit for clarity, also released on drop)
        file.unlock()?;

        // Update in-memory cache
        self.recent.push(entry);
        if self.recent.len() > 100 {
            self.recent.remove(0);
        }

        // Recompute patterns if failure
        self.compute_failure_patterns();

        Ok(())
    }

    /// Load recent entries from the log (optimized for large files)
    ///
    /// Uses shared lock for concurrent read safety and reads efficiently
    /// by estimating position for large files.
    fn load_recent(&mut self, limit: usize) {
        if !Path::new(&self.log_path).exists() {
            return;
        }

        let file = match File::open(&self.log_path) {
            Ok(f) => f,
            Err(_) => return,
        };

        // Acquire shared lock for reading (allows concurrent readers)
        if file.lock_shared().is_err() {
            return;
        }

        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(_) => {
                let _ = file.unlock();
                return;
            }
        };

        let file_size = metadata.len();

        // For small files (< 100KB), read entire file
        // For large files, estimate position and read from there
        let entries = if file_size < 100_000 {
            self.load_all_entries(&file)
        } else {
            self.load_entries_optimized(&file, file_size, limit)
        };

        let _ = file.unlock();

        // Take last N entries
        let start = entries.len().saturating_sub(limit);
        self.recent = entries[start..].to_vec();
    }

    /// Load all entries from file (for small files)
    fn load_all_entries(&self, file: &File) -> Vec<MemoryEntry> {
        let reader = BufReader::new(file);
        reader
            .lines()
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect()
    }

    /// Load entries from estimated position (for large files)
    ///
    /// Estimates ~500 bytes per entry average, seeks near end, and reads forward.
    /// Falls back to full read if estimation is off.
    fn load_entries_optimized(
        &self,
        file: &File,
        file_size: u64,
        limit: usize,
    ) -> Vec<MemoryEntry> {
        // Estimate: average entry ~500 bytes, seek to get ~2x limit entries
        let estimated_bytes_needed = (limit * 2 * 500) as u64;
        let seek_position = file_size.saturating_sub(estimated_bytes_needed);

        let mut file = file;

        // Seek to estimated position
        if file.seek(SeekFrom::Start(seek_position)).is_err() {
            return self.load_all_entries(file);
        }

        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Skip first (likely partial) line if we seeked mid-file
        if seek_position > 0 {
            let _ = lines.next();
        }

        let entries: Vec<MemoryEntry> = lines
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();

        // If we didn't get enough entries, fall back to full read
        if entries.len() < limit && seek_position > 0 {
            // Reset and read all
            let file = match File::open(&self.log_path) {
                Ok(f) => f,
                Err(_) => return entries,
            };
            return self.load_all_entries(&file);
        }

        entries
    }

    /// Compute failure patterns from recent entries
    fn compute_failure_patterns(&mut self) {
        self.failure_patterns.clear();

        for entry in &self.recent {
            if entry.memory_type != MemoryType::Failure {
                continue;
            }

            // Extract pattern from error message
            if let Some(error) = &entry.error {
                let pattern = extract_error_pattern(error);
                let fp = self
                    .failure_patterns
                    .entry(pattern.clone())
                    .or_insert_with(|| FailurePattern {
                        pattern: pattern.clone(),
                        count: 0,
                        affected_tasks: Vec::new(),
                        suggestion: suggest_for_pattern(&pattern),
                    });

                fp.count += 1;
                if let Some(task_id) = &entry.task_id {
                    if !fp.affected_tasks.contains(task_id) {
                        fp.affected_tasks.push(task_id.clone());
                    }
                }
            }
        }
    }

    /// Check if a task has previous failures
    pub fn has_failures(&self, task_id: &str) -> bool {
        self.recent
            .iter()
            .any(|e| e.memory_type == MemoryType::Failure && e.task_id.as_deref() == Some(task_id))
    }

    /// Count failures for a task
    pub fn failure_count(&self, task_id: &str) -> u32 {
        self.recent
            .iter()
            .filter(|e| {
                e.memory_type == MemoryType::Failure && e.task_id.as_deref() == Some(task_id)
            })
            .count() as u32
    }

    /// Get failures for a task
    pub fn get_failures(&self, task_id: &str) -> Vec<&MemoryEntry> {
        self.recent
            .iter()
            .filter(|e| {
                e.memory_type == MemoryType::Failure && e.task_id.as_deref() == Some(task_id)
            })
            .collect()
    }

    /// Search entries by tag
    pub fn search_by_tag(&self, tag: &str) -> Vec<&MemoryEntry> {
        self.recent
            .iter()
            .filter(|e| e.tags.iter().any(|t| t.contains(tag)))
            .collect()
    }

    /// Get active failure patterns (recurring issues)
    pub fn get_failure_patterns(&self) -> Vec<&FailurePattern> {
        self.failure_patterns
            .values()
            .filter(|p| p.count >= 2) // Only patterns that occurred 2+ times
            .collect()
    }

    /// Check if an error matches a known pattern
    pub fn matches_known_pattern(&self, error: &str) -> Option<&FailurePattern> {
        let pattern = extract_error_pattern(error);
        self.failure_patterns.get(&pattern)
    }

    /// Get workarounds for a pattern
    pub fn get_workarounds(&self, pattern: &str) -> Vec<&MemoryEntry> {
        self.recent
            .iter()
            .filter(|e| {
                e.memory_type == MemoryType::Workaround
                    && e.tags.iter().any(|t| t.contains(pattern))
            })
            .collect()
    }

    /// Generate context summary for current session
    pub fn compile_context(&self, epic_id: Option<&str>) -> String {
        let mut context = String::new();

        // Add relevant failures
        let failures: Vec<_> = self
            .recent
            .iter()
            .filter(|e| {
                e.memory_type == MemoryType::Failure
                    && (epic_id.is_none() || e.epic_id.as_deref() == epic_id)
            })
            .collect();

        if !failures.is_empty() {
            context.push_str("## Recent Failures\n");
            for f in failures.iter().take(5) {
                context.push_str(&format!(
                    "- {}: {}\n",
                    f.task_id.as_deref().unwrap_or("?"),
                    f.summary
                ));
                if let Some(err) = &f.error {
                    context.push_str(&format!("  Error: {}\n", truncate(err, 100)));
                }
            }
            context.push('\n');
        }

        // Add active patterns to avoid
        let patterns = self.get_failure_patterns();
        if !patterns.is_empty() {
            context.push_str("## Patterns to Avoid\n");
            for p in patterns.iter().take(3) {
                context.push_str(&format!("- {} (occurred {} times)\n", p.pattern, p.count));
                context.push_str(&format!("  Suggestion: {}\n", p.suggestion));
            }
            context.push('\n');
        }

        // Add recent workarounds
        let workarounds: Vec<_> = self
            .recent
            .iter()
            .filter(|e| e.memory_type == MemoryType::Workaround)
            .collect();

        if !workarounds.is_empty() {
            context.push_str("## Known Workarounds\n");
            for w in workarounds.iter().take(3) {
                context.push_str(&format!("- {}\n", w.summary));
            }
        }

        context
    }
}

// Helper functions

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("mem-{:x}", duration.as_micros())
}

fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    // Simple ISO-like timestamp
    format!("{}", duration.as_secs())
}

/// Extract a normalized pattern from an error message
fn extract_error_pattern(error: &str) -> String {
    let error_lower = error.to_lowercase();

    // Common error patterns
    if error_lower.contains("not found") {
        return "resource_not_found".to_string();
    }
    if error_lower.contains("permission denied") {
        return "permission_denied".to_string();
    }
    if error_lower.contains("timeout") || error_lower.contains("timed out") {
        return "timeout".to_string();
    }
    if error_lower.contains("connection") && error_lower.contains("refused") {
        return "connection_refused".to_string();
    }
    if error_lower.contains("syntax error") || error_lower.contains("parse error") {
        return "syntax_error".to_string();
    }
    if error_lower.contains("type error") || error_lower.contains("type mismatch") {
        return "type_error".to_string();
    }
    if error_lower.contains("test failed") || error_lower.contains("assertion failed") {
        return "test_failure".to_string();
    }
    if error_lower.contains("compile") && error_lower.contains("error") {
        return "compile_error".to_string();
    }
    if error_lower.contains("out of memory") || error_lower.contains("oom") {
        return "memory_error".to_string();
    }
    if error_lower.contains("deadlock") || error_lower.contains("race condition") {
        return "concurrency_error".to_string();
    }

    // Default: first significant words
    let words: Vec<&str> = error.split_whitespace().take(3).collect();
    words.join("_").to_lowercase()
}

/// Suggest actions for known patterns
fn suggest_for_pattern(pattern: &str) -> String {
    match pattern {
        "resource_not_found" => "Verify path/ID exists before accessing".to_string(),
        "permission_denied" => {
            "Check file permissions or run with appropriate privileges".to_string()
        }
        "timeout" => "Increase timeout or check network connectivity".to_string(),
        "connection_refused" => "Verify service is running and port is correct".to_string(),
        "syntax_error" => "Check for missing brackets, semicolons, or typos".to_string(),
        "type_error" => "Verify type annotations and function signatures".to_string(),
        "test_failure" => "Review test expectations and implementation".to_string(),
        "compile_error" => "Fix compilation errors before running tests".to_string(),
        "memory_error" => "Reduce data size or optimize memory usage".to_string(),
        "concurrency_error" => "Add proper synchronization or use async patterns".to_string(),
        _ => "Review error details and consider alternative approaches".to_string(),
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_memory_entry_creation() {
        let success = MemoryEntry::success("task-1", "Completed login feature");
        assert_eq!(success.memory_type, MemoryType::Success);
        assert_eq!(success.task_id, Some("task-1".to_string()));

        let failure = MemoryEntry::failure("task-2", "Build failed", "error: syntax error");
        assert_eq!(failure.memory_type, MemoryType::Failure);
        assert!(failure.error.is_some());
    }

    #[test]
    fn test_memory_with_tags() {
        let entry = MemoryEntry::success("task-1", "Test")
            .with_tags(vec!["auth".to_string(), "login".to_string()]);
        assert_eq!(entry.tags.len(), 2);
    }

    #[test]
    fn test_procedural_memory_append() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        let entry = MemoryEntry::success("task-1", "Test success");
        memory.append(entry).unwrap();

        assert_eq!(memory.recent.len(), 1);
    }

    #[test]
    fn test_failure_counting() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        memory
            .append(MemoryEntry::failure("task-1", "First fail", "error 1"))
            .unwrap();
        memory
            .append(MemoryEntry::failure("task-1", "Second fail", "error 2"))
            .unwrap();
        memory
            .append(MemoryEntry::success("task-1", "Finally worked"))
            .unwrap();

        assert_eq!(memory.failure_count("task-1"), 2);
        assert!(memory.has_failures("task-1"));
    }

    #[test]
    fn test_pattern_extraction() {
        assert_eq!(
            extract_error_pattern("File not found: /path/to/file"),
            "resource_not_found"
        );
        assert_eq!(
            extract_error_pattern("Permission denied when accessing /etc"),
            "permission_denied"
        );
        assert_eq!(
            extract_error_pattern("Connection timed out after 30s"),
            "timeout"
        );
        assert_eq!(
            extract_error_pattern("Test failed: expected 5, got 3"),
            "test_failure"
        );
    }

    #[test]
    fn test_failure_patterns() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add multiple failures with same pattern
        memory
            .append(MemoryEntry::failure("task-1", "Fail 1", "File not found"))
            .unwrap();
        memory
            .append(MemoryEntry::failure("task-2", "Fail 2", "Config not found"))
            .unwrap();

        let patterns = memory.get_failure_patterns();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].pattern, "resource_not_found");
        assert_eq!(patterns[0].count, 2);
    }
}
