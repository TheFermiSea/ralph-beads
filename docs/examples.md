# Ralph-Beads Examples

Real-world workflow examples demonstrating ralph-beads usage patterns.

## Example 1: New Feature Implementation

**Scenario:** Add user authentication to an Express.js API.

### Planning Phase

```bash
# Start planning mode
/ralph-beads --mode plan "Add JWT authentication to API endpoints"

# Ralph-beads will:
# 1. Create proto epic with ralph + template labels
# 2. Set mode=planning
# 3. Start ralph-loop with PLAN_READY completion promise
```

**What happens during planning:**
- Agent explores codebase to understand current structure
- Creates tasks with dependencies:
  1. Add passport and jwt dependencies
  2. Create auth middleware
  3. Add login endpoint
  4. Add token refresh endpoint
  5. Protect existing routes
  6. Add tests for auth flow
- Sets dependencies: 2→1, 3→2, 4→3, 5→4, 6→5
- Outputs `<promise>PLAN_READY</promise>` when complete

### Building Phase

```bash
# Pour proto into molecule and start building
/ralph-beads --mode build --epic rb-xxx

# Or resume an existing molecule
/ralph-beads --mol rb-mol-yyy
```

**What happens during building:**
- Agent runs `bd prime` each iteration (fresh context)
- Gets next task from `bd ready --mol <id> --limit 1`
- Implements, tests, commits
- Closes task (unblocks dependents)
- Repeats until 100% complete

### Error Recovery

```bash
# If loop times out or is cancelled
/ralph-status                           # Check progress
/ralph-beads --mol rb-mol-yyy           # Resume where left off

# If a task is stuck (circuit breaker triggered)
bd list --parent=rb-xxx --status=blocked  # See blocked tasks
bd comments list <blocked-task-id>         # See failure reasons
# Fix the issue manually, then:
bd update <blocked-task-id> --status=open  # Unblock
/ralph-beads --mol rb-mol-yyy              # Resume
```

---

## Example 2: Bug Fix with Discovery

**Scenario:** Fix a race condition that causes intermittent test failures.

### Quick Start

```bash
# For bug fixes, building mode is usually sufficient
/ralph-beads "Fix race condition in WebSocket connection handler"
```

### Discovering Additional Work

During building, agent finds a related issue in the cleanup code:

```bash
# Create ephemeral task (not synced to git)
bd create --ephemeral --title="Fix cleanup handler memory leak"

# Do the quick fix
# ... make changes ...

# Close immediately
bd close <ephemeral-task-id>

# Continue with main task
```

### Progress Check

```bash
/ralph-status                    # Quick overview
/ralph-status --verbose          # Include iteration history

# Or directly with beads:
bd --no-daemon mol progress <mol-id>
bd graph <epic-id>
```

---

## Example 3: Large Refactoring Project

**Scenario:** Migrate from callbacks to async/await across the codebase.

### Planning with Dependencies

```bash
/ralph-beads --mode plan --priority 1 "Migrate codebase from callbacks to async/await"
```

Planning creates a structured task graph:
```
1. Create async utility helpers              [P1] (no deps)
2. Migrate core/database.js                  [P1] (depends on 1)
3. Migrate core/api-client.js                [P1] (depends on 1)
4. Migrate services/auth.js                  [P2] (depends on 2, 3)
5. Migrate services/user.js                  [P2] (depends on 2)
6. Migrate routes/*.js                       [P2] (depends on 4, 5)
7. Update tests                              [P3] (depends on 6)
8. Remove callback compat shims              [P3] (depends on 7)
```

### Parallel Work (Advanced)

Multiple agents can work on independent tasks:

```bash
# Terminal 1: Work on task 2
cd $(git worktree add ../refactor-db refactor-db)
/ralph-beads --mol rb-mol-xxx  # Agent picks task 2

# Terminal 2: Work on task 3 (independent)
cd $(git worktree add ../refactor-api refactor-api)
/ralph-beads --mol rb-mol-xxx  # Agent picks task 3
```

### Resuming After Break

```bash
# Check what was in progress
bd list --parent=<epic-id> --status=in_progress

# Get current molecule state
bd --no-daemon mol progress <mol-id>
bd --no-daemon mol current <mol-id>

# Resume
/ralph-beads --mol <mol-id> --max-iterations 40
```

---

## Example 4: Handling Failures

### Circuit Breaker Triggered

Task fails twice, gets blocked:

```bash
# See the blocked task
bd list --parent=<epic-id> --status=blocked
# Output: rb-task-123 - Implement OAuth flow

# Check what went wrong
bd comments list rb-task-123
# Output:
#   [ATTEMPT:1] Failed: Missing OAUTH_CLIENT_ID env var
#   [ATTEMPT:2] Failed: Missing OAUTH_CLIENT_ID env var. CIRCUIT BREAKER TRIGGERED.

# Fix: Add the env var to .env.example and your environment
# Then unblock:
bd update rb-task-123 --status=open
bd comments add rb-task-123 "[UNBLOCKED] Added OAUTH_CLIENT_ID to .env.example"

# Resume the molecule
/ralph-beads --mol <mol-id>
```

### Dependency Cycle Detected

```bash
# During planning validation, cycle detected
bd graph <epic-id>
# Output shows: CYCLE DETECTED: task-a -> task-b -> task-c -> task-a

# Fix: Remove one dependency to break the cycle
bd dep remove task-c task-a

# Verify fixed
bd graph <epic-id>
# Output: Valid DAG
```

---

## Example 5: Cancellation and Recovery

### Graceful Cancellation

```bash
# Need to stop and switch to urgent bug fix
/ralph-cancel --reason "Pivoting to urgent security patch"

# Output:
# Cancelled Ralph-Beads loop.
# Epic: rb-xxx - Feature implementation
# Progress: 4/8 tasks complete (50%)
# In-progress: rb-task-456 - Add validation (preserved)
# Resume with: /ralph-beads --epic rb-xxx
```

### Recovering After Crash/Timeout

```bash
# Find your epic
bd list --type=epic --label=ralph --status=open

# Check state
/ralph-status rb-xxx

# Resume with higher iteration limit
/ralph-beads --mol <mol-id> --max-iterations 50
```

---

## Quick Reference

| Command | When to Use |
|---------|-------------|
| `/ralph-beads --mode plan "task"` | Start new work with planning |
| `/ralph-beads "task"` | Quick bug fix (skip planning) |
| `/ralph-beads --epic <id>` | Resume from proto |
| `/ralph-beads --mol <id>` | Resume from molecule |
| `/ralph-status` | Check progress |
| `/ralph-cancel` | Gracefully stop |
| `bd ready --mol <id>` | See next tasks |
| `bd --no-daemon mol progress <id>` | Completion percentage |
