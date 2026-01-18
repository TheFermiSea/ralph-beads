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

/// Configuration for log rotation
#[derive(Debug, Clone)]
pub struct RotationConfig {
    /// Max entries before rotation (default: 10000)
    pub max_entries: usize,
    /// Max file size in bytes (default: 10MB)
    pub max_file_size: u64,
    /// Number of archives to keep (default: 3)
    pub archive_count: usize,
    /// Compress archives (default: true)
    pub compress: bool,
}

impl Default for RotationConfig {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            max_file_size: 10 * 1024 * 1024, // 10MB
            archive_count: 3,
            compress: true,
        }
    }
}

impl RotationConfig {
    /// Create a new rotation config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max entries
    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Set max file size
    pub fn with_max_file_size(mut self, max_file_size: u64) -> Self {
        self.max_file_size = max_file_size;
        self
    }

    /// Set archive count
    pub fn with_archive_count(mut self, archive_count: usize) -> Self {
        self.archive_count = archive_count;
        self
    }

    /// Set compression flag
    pub fn with_compress(mut self, compress: bool) -> Self {
        self.compress = compress;
        self
    }
}

/// Result of a rotation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationResult {
    /// Whether rotation occurred
    pub rotated: bool,
    /// Number of entries archived
    pub entries_archived: usize,
    /// New file size after rotation
    pub new_file_size: u64,
    /// Number of archives pruned
    pub archives_pruned: usize,
}

impl RotationResult {
    /// Create a result indicating no rotation occurred
    pub fn no_rotation() -> Self {
        Self {
            rotated: false,
            entries_archived: 0,
            new_file_size: 0,
            archives_pruned: 0,
        }
    }
}

/// Statistics about the log file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStats {
    /// Current file size in bytes
    pub file_size: u64,
    /// Number of entries in the log
    pub entry_count: usize,
    /// Oldest entry timestamp (if available)
    pub oldest_entry: Option<String>,
    /// Newest entry timestamp (if available)
    pub newest_entry: Option<String>,
    /// Number of archive files present
    pub archive_count: usize,
    /// Total size of all archives
    pub total_archive_size: u64,
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
    ///
    /// Automatically checks for rotation after each write using default thresholds.
    /// Use `append_with_rotation` for custom rotation configuration.
    pub fn append(&mut self, entry: MemoryEntry) -> std::io::Result<()> {
        self.append_internal(entry, Some(&RotationConfig::default()))
    }

    /// Append an entry with custom rotation configuration
    pub fn append_with_rotation(
        &mut self,
        entry: MemoryEntry,
        rotation_config: Option<&RotationConfig>,
    ) -> std::io::Result<()> {
        self.append_internal(entry, rotation_config)
    }

    /// Internal append implementation
    fn append_internal(
        &mut self,
        entry: MemoryEntry,
        rotation_config: Option<&RotationConfig>,
    ) -> std::io::Result<()> {
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

        // Check for rotation if config provided (log errors but don't fail the append)
        if let Some(config) = rotation_config {
            if let Err(e) = self.rotate_if_needed(config) {
                // Log rotation errors but don't fail the append
                eprintln!("Warning: log rotation check failed: {}", e);
            }
        }

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

    // ==================== Log Rotation Methods ====================

    /// Get statistics about the log file
    pub fn get_log_stats(&self) -> LogStats {
        let mut stats = LogStats {
            file_size: 0,
            entry_count: 0,
            oldest_entry: None,
            newest_entry: None,
            archive_count: 0,
            total_archive_size: 0,
        };

        // Get main log file stats
        if let Ok(metadata) = fs::metadata(&self.log_path) {
            stats.file_size = metadata.len();
        }

        // Count entries in the file
        if let Ok(file) = File::open(&self.log_path) {
            if file.lock_shared().is_ok() {
                let reader = BufReader::new(&file);
                let entries: Vec<MemoryEntry> = reader
                    .lines()
                    .filter_map(|line| line.ok())
                    .filter_map(|line| serde_json::from_str(&line).ok())
                    .collect();

                stats.entry_count = entries.len();

                if let Some(first) = entries.first() {
                    stats.oldest_entry = Some(first.timestamp.clone());
                }
                if let Some(last) = entries.last() {
                    stats.newest_entry = Some(last.timestamp.clone());
                }

                let _ = file.unlock();
            }
        }

        // Count and size archives
        if let Some(parent) = Path::new(&self.log_path).parent() {
            if let Some(file_name) = Path::new(&self.log_path).file_name() {
                let base_name = file_name.to_string_lossy();
                if let Ok(entries) = fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with(&*base_name) && name != *base_name {
                            // This is an archive file
                            stats.archive_count += 1;
                            if let Ok(meta) = entry.metadata() {
                                stats.total_archive_size += meta.len();
                            }
                        }
                    }
                }
            }
        }

        stats
    }

    /// Check if rotation is needed based on thresholds
    fn needs_rotation(&self, config: &RotationConfig) -> bool {
        // Check file size
        if let Ok(metadata) = fs::metadata(&self.log_path) {
            if metadata.len() >= config.max_file_size {
                return true;
            }
        }

        // Check entry count
        if let Ok(file) = File::open(&self.log_path) {
            if file.lock_shared().is_ok() {
                let reader = BufReader::new(&file);
                let count = reader
                    .lines()
                    .filter_map(|line| line.ok())
                    .filter(|line| serde_json::from_str::<MemoryEntry>(line).is_ok())
                    .count();
                let _ = file.unlock();
                if count >= config.max_entries {
                    return true;
                }
            }
        }

        false
    }

    /// Rotate the log if thresholds are exceeded
    pub fn rotate_if_needed(&mut self, config: &RotationConfig) -> std::io::Result<RotationResult> {
        if !self.needs_rotation(config) {
            return Ok(RotationResult::no_rotation());
        }
        self.force_rotate(config)
    }

    /// Force rotation regardless of thresholds
    pub fn force_rotate(&mut self, config: &RotationConfig) -> std::io::Result<RotationResult> {
        if !Path::new(&self.log_path).exists() {
            return Ok(RotationResult::no_rotation());
        }

        // Open and lock the file for reading
        let file = OpenOptions::new().read(true).open(&self.log_path)?;
        file.lock_exclusive()?;

        // Read all entries and the raw content for archiving
        let reader = BufReader::new(&file);
        let entries: Vec<MemoryEntry> = reader
            .lines()
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();

        let entries_archived = entries.len();

        if entries_archived == 0 {
            file.unlock()?;
            return Ok(RotationResult::no_rotation());
        }

        // Generate archive path
        let archive_path = self.next_archive_path();

        // Release the lock BEFORE archiving to avoid deadlock
        // (archive_to_path also needs to lock the file)
        file.unlock()?;
        drop(file);

        // Archive the current log (will acquire its own lock)
        self.archive_to_path(&archive_path, config.compress)?;

        // Now truncate the main log file by recreating it
        // Use exclusive lock during truncation
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.log_path)?;
        file.lock_exclusive()?;
        file.unlock()?;
        drop(file);

        // Clean up old archives
        let archives_pruned = self.cleanup_archives_internal(config.archive_count)?;

        // Clear in-memory cache
        self.recent.clear();
        self.failure_patterns.clear();

        // Get new file size
        let new_file_size = fs::metadata(&self.log_path).map(|m| m.len()).unwrap_or(0);

        Ok(RotationResult {
            rotated: true,
            entries_archived,
            new_file_size,
            archives_pruned,
        })
    }

    /// Prune old entries in-place, keeping the most recent N entries
    pub fn prune_old_entries(&mut self, keep_count: usize) -> std::io::Result<usize> {
        if !Path::new(&self.log_path).exists() {
            return Ok(0);
        }

        // Open and lock the file
        let file = OpenOptions::new().read(true).open(&self.log_path)?;
        file.lock_exclusive()?;

        // Read all entries
        let reader = BufReader::new(&file);
        let entries: Vec<MemoryEntry> = reader
            .lines()
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();

        file.unlock()?;
        drop(file);

        if entries.len() <= keep_count {
            return Ok(0);
        }

        let pruned_count = entries.len() - keep_count;
        let entries_to_keep = &entries[pruned_count..];

        // Rewrite the file with only kept entries
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.log_path)?;
        file.lock_exclusive()?;

        let mut writer = std::io::BufWriter::new(&file);
        for entry in entries_to_keep {
            let json = serde_json::to_string(entry).unwrap();
            writeln!(writer, "{}", json)?;
        }
        writer.flush()?;

        file.unlock()?;

        // Reload recent entries
        self.load_recent(100);
        self.compute_failure_patterns();

        Ok(pruned_count)
    }

    /// Archive the log to a specific path
    pub fn archive_log(&self, archive_path: &str) -> std::io::Result<()> {
        self.archive_to_path(archive_path, false)
    }

    /// Archive with optional compression
    fn archive_to_path(&self, archive_path: &str, compress: bool) -> std::io::Result<()> {
        if !Path::new(&self.log_path).exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Log file does not exist",
            ));
        }

        if compress {
            // Use gzip compression
            use flate2::write::GzEncoder;
            use flate2::Compression;

            let gz_path = if archive_path.ends_with(".gz") {
                archive_path.to_string()
            } else {
                format!("{}.gz", archive_path)
            };

            let input = File::open(&self.log_path)?;
            input.lock_shared()?;

            let output = File::create(&gz_path)?;
            let mut encoder = GzEncoder::new(output, Compression::default());
            let mut reader = BufReader::new(&input);
            std::io::copy(&mut reader, &mut encoder)?;
            encoder.finish()?;

            input.unlock()?;
        } else {
            // Simple copy
            fs::copy(&self.log_path, archive_path)?;
        }

        Ok(())
    }

    /// Clean up old archives, keeping only the specified count
    pub fn cleanup_archives(&self, keep_count: usize) -> std::io::Result<usize> {
        self.cleanup_archives_internal(keep_count)
    }

    /// Internal archive cleanup
    fn cleanup_archives_internal(&self, keep_count: usize) -> std::io::Result<usize> {
        let archives = self.list_archives()?;

        if archives.len() <= keep_count {
            return Ok(0);
        }

        // Sort by name (which includes number, so older ones first)
        let mut archives_with_num: Vec<(String, usize)> = archives
            .iter()
            .filter_map(|path| {
                let name = Path::new(path).file_name()?.to_string_lossy().to_string();
                // Extract number from name like "memory.jsonl.1" or "memory.jsonl.1.gz"
                let parts: Vec<&str> = name.split('.').collect();
                for (i, part) in parts.iter().enumerate() {
                    if let Ok(num) = part.parse::<usize>() {
                        return Some((path.clone(), num));
                    }
                    // Check if it's "N.gz" pattern
                    if *part == "gz" && i > 0 {
                        if let Ok(num) = parts[i - 1].parse::<usize>() {
                            return Some((path.clone(), num));
                        }
                    }
                }
                None
            })
            .collect();

        // Sort by number (highest = newest, lowest = oldest)
        archives_with_num.sort_by(|a, b| b.1.cmp(&a.1));

        let _to_remove = archives_with_num.len().saturating_sub(keep_count);
        let mut removed = 0;

        // Remove oldest archives (those at the end after sorting by number descending)
        for (path, _) in archives_with_num.iter().skip(keep_count) {
            if fs::remove_file(path).is_ok() {
                removed += 1;
            }
        }

        Ok(removed)
    }

    /// List all archive files
    fn list_archives(&self) -> std::io::Result<Vec<String>> {
        let mut archives = Vec::new();

        if let Some(parent) = Path::new(&self.log_path).parent() {
            if let Some(file_name) = Path::new(&self.log_path).file_name() {
                let base_name = file_name.to_string_lossy();
                for entry in fs::read_dir(parent)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with(&*base_name) && name != *base_name {
                        archives.push(entry.path().to_string_lossy().to_string());
                    }
                }
            }
        }

        Ok(archives)
    }

    /// Generate the next archive path
    fn next_archive_path(&self) -> String {
        let archives = self.list_archives().unwrap_or_default();
        let max_num = archives
            .iter()
            .filter_map(|path| {
                let name = Path::new(path).file_name()?.to_string_lossy().to_string();
                // Extract number from patterns like "memory.jsonl.1" or "memory.jsonl.1.gz"
                let parts: Vec<&str> = name.split('.').collect();
                for (i, part) in parts.iter().enumerate() {
                    if let Ok(num) = part.parse::<usize>() {
                        return Some(num);
                    }
                    if *part == "gz" && i > 0 {
                        if let Ok(num) = parts[i - 1].parse::<usize>() {
                            return Some(num);
                        }
                    }
                }
                None
            })
            .max()
            .unwrap_or(0);

        format!("{}.{}", self.log_path, max_num + 1)
    }

    /// Get the log path
    pub fn log_path(&self) -> &str {
        &self.log_path
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

    // ==================== Rotation Tests ====================

    #[test]
    fn test_rotation_config_defaults() {
        let config = RotationConfig::default();
        assert_eq!(config.max_entries, 10_000);
        assert_eq!(config.max_file_size, 10 * 1024 * 1024);
        assert_eq!(config.archive_count, 3);
        assert!(config.compress);
    }

    #[test]
    fn test_rotation_config_builder() {
        let config = RotationConfig::new()
            .with_max_entries(1000)
            .with_max_file_size(1024 * 1024)
            .with_archive_count(5)
            .with_compress(false);

        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.max_file_size, 1024 * 1024);
        assert_eq!(config.archive_count, 5);
        assert!(!config.compress);
    }

    #[test]
    fn test_get_log_stats_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let memory = ProceduralMemory::new(path.to_str().unwrap());

        let stats = memory.get_log_stats();
        assert_eq!(stats.file_size, 0);
        assert_eq!(stats.entry_count, 0);
        assert!(stats.oldest_entry.is_none());
        assert!(stats.newest_entry.is_none());
        assert_eq!(stats.archive_count, 0);
    }

    #[test]
    fn test_get_log_stats_with_entries() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add some entries (use append_with_rotation to skip auto-rotation)
        memory
            .append_with_rotation(MemoryEntry::success("task-1", "First"), None)
            .unwrap();
        memory
            .append_with_rotation(MemoryEntry::success("task-2", "Second"), None)
            .unwrap();
        memory
            .append_with_rotation(MemoryEntry::success("task-3", "Third"), None)
            .unwrap();

        let stats = memory.get_log_stats();
        assert!(stats.file_size > 0);
        assert_eq!(stats.entry_count, 3);
        assert!(stats.oldest_entry.is_some());
        assert!(stats.newest_entry.is_some());
    }

    #[test]
    fn test_prune_old_entries() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add 10 entries
        for i in 0..10 {
            memory
                .append_with_rotation(
                    MemoryEntry::success(&format!("task-{}", i), &format!("Entry {}", i)),
                    None,
                )
                .unwrap();
        }

        // Verify 10 entries
        let stats_before = memory.get_log_stats();
        assert_eq!(stats_before.entry_count, 10);

        // Prune to keep 5
        let pruned = memory.prune_old_entries(5).unwrap();
        assert_eq!(pruned, 5);

        // Verify 5 remaining
        let stats_after = memory.get_log_stats();
        assert_eq!(stats_after.entry_count, 5);
    }

    #[test]
    fn test_prune_keeps_newest() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add entries with unique task IDs
        for i in 0..5 {
            memory
                .append_with_rotation(
                    MemoryEntry::success(&format!("task-{}", i), &format!("Entry {}", i)),
                    None,
                )
                .unwrap();
        }

        // Prune to keep 2 (should keep task-3 and task-4)
        memory.prune_old_entries(2).unwrap();

        // Check that recent cache has the newest entries
        assert_eq!(memory.recent.len(), 2);
        // The most recent entries should be task-3 and task-4
        assert_eq!(memory.recent[0].task_id, Some("task-3".to_string()));
        assert_eq!(memory.recent[1].task_id, Some("task-4".to_string()));
    }

    #[test]
    fn test_archive_log_uncompressed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let archive_path = dir.path().join("archive.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add an entry
        memory
            .append_with_rotation(MemoryEntry::success("task-1", "Test"), None)
            .unwrap();

        // Archive
        memory.archive_log(archive_path.to_str().unwrap()).unwrap();

        // Verify archive exists and has content
        let archive_content = std::fs::read_to_string(&archive_path).unwrap();
        assert!(archive_content.contains("task-1"));
    }

    #[test]
    fn test_force_rotate_creates_archive() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add entries
        for i in 0..5 {
            memory
                .append_with_rotation(MemoryEntry::success(&format!("task-{}", i), "Test"), None)
                .unwrap();
        }

        // Force rotate with no compression for easier testing
        let config = RotationConfig::new().with_compress(false);
        let result = memory.force_rotate(&config).unwrap();

        assert!(result.rotated);
        assert_eq!(result.entries_archived, 5);
        assert_eq!(result.new_file_size, 0);

        // Verify archive was created
        let stats = memory.get_log_stats();
        assert_eq!(stats.archive_count, 1);

        // Verify main log is empty
        assert_eq!(stats.entry_count, 0);
    }

    #[test]
    fn test_rotate_if_needed_by_entries() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add 5 entries
        for i in 0..5 {
            memory
                .append_with_rotation(MemoryEntry::success(&format!("task-{}", i), "Test"), None)
                .unwrap();
        }

        // Config with max_entries = 5, should trigger rotation
        let config = RotationConfig::new()
            .with_max_entries(5)
            .with_compress(false);

        let result = memory.rotate_if_needed(&config).unwrap();
        assert!(result.rotated);
        assert_eq!(result.entries_archived, 5);
    }

    #[test]
    fn test_rotate_if_needed_under_threshold() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add 3 entries
        for i in 0..3 {
            memory
                .append_with_rotation(MemoryEntry::success(&format!("task-{}", i), "Test"), None)
                .unwrap();
        }

        // Config with max_entries = 10, should NOT trigger rotation
        let config = RotationConfig::new()
            .with_max_entries(10)
            .with_compress(false);

        let result = memory.rotate_if_needed(&config).unwrap();
        assert!(!result.rotated);
    }

    #[test]
    fn test_cleanup_archives() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Create multiple archives by rotating multiple times
        let config = RotationConfig::new()
            .with_max_entries(2)
            .with_archive_count(10) // Don't auto-cleanup
            .with_compress(false);

        // Add and rotate multiple times to create archives
        for batch in 0..5 {
            for i in 0..2 {
                memory
                    .append_with_rotation(
                        MemoryEntry::success(&format!("task-{}-{}", batch, i), "Test"),
                        None,
                    )
                    .unwrap();
            }
            memory.force_rotate(&config).unwrap();
        }

        // Should have 5 archives
        let stats_before = memory.get_log_stats();
        assert_eq!(stats_before.archive_count, 5);

        // Cleanup to keep only 2
        let removed = memory.cleanup_archives(2).unwrap();
        assert_eq!(removed, 3);

        // Verify only 2 remain
        let stats_after = memory.get_log_stats();
        assert_eq!(stats_after.archive_count, 2);
    }

    #[test]
    fn test_rotation_result_no_rotation() {
        let result = RotationResult::no_rotation();
        assert!(!result.rotated);
        assert_eq!(result.entries_archived, 0);
        assert_eq!(result.new_file_size, 0);
        assert_eq!(result.archives_pruned, 0);
    }

    #[test]
    fn test_multiple_rotations_increment_archive_number() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        let config = RotationConfig::new()
            .with_archive_count(5)
            .with_compress(false);

        // First rotation
        memory
            .append_with_rotation(MemoryEntry::success("task-1", "Test"), None)
            .unwrap();
        memory.force_rotate(&config).unwrap();

        // Second rotation
        memory
            .append_with_rotation(MemoryEntry::success("task-2", "Test"), None)
            .unwrap();
        memory.force_rotate(&config).unwrap();

        // Third rotation
        memory
            .append_with_rotation(MemoryEntry::success("task-3", "Test"), None)
            .unwrap();
        memory.force_rotate(&config).unwrap();

        // Should have 3 archives with incrementing numbers
        let archives = memory.list_archives().unwrap();
        assert_eq!(archives.len(), 3);

        // Verify archive names contain 1, 2, 3
        let names: Vec<String> = archives
            .iter()
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert!(names.iter().any(|n| n.contains(".1")));
        assert!(names.iter().any(|n| n.contains(".2")));
        assert!(names.iter().any(|n| n.contains(".3")));
    }

    #[test]
    fn test_compressed_archive() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("memory.jsonl");
        let mut memory = ProceduralMemory::new(path.to_str().unwrap());

        // Add entries
        for i in 0..5 {
            memory
                .append_with_rotation(MemoryEntry::success(&format!("task-{}", i), "Test"), None)
                .unwrap();
        }

        // Force rotate with compression
        let config = RotationConfig::new().with_compress(true);
        let result = memory.force_rotate(&config).unwrap();

        assert!(result.rotated);

        // Check that a .gz file was created
        let archives = memory.list_archives().unwrap();
        assert_eq!(archives.len(), 1);
        assert!(archives[0].ends_with(".gz"));
    }
}
