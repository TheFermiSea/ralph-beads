# Testing Guide for ralph-beads

This document covers testing procedures for both the Claude Code plugin and the OpenCode plugin implementations.

## Quick Validation

Run the automated test suite:

```bash
./tests/run-tests.sh
```

This validates:
- Shell script syntax
- Embedded bash block validity
- Complexity heuristics
- Signal handling and cleanup
- Prompt snapshot invariants

## Claude Code Plugin Testing

### Prerequisites

```bash
# Install plugin locally
cd ~/code/ralph-beads
claude plugins install .

# Verify beads is initialized
bd info
```

### Test 1: Dry Run (No Side Effects)

**Purpose:** Verify plugin loads and parses arguments correctly.

```bash
# In any project directory with beads initialized
/ralph-beads --dry-run --mode plan "Test task description"
```

**Expected:** Output showing mode, complexity, max-iterations without creating any beads issues.

### Test 2: Planning Mode

**Purpose:** Verify planning mode creates proto epic with tasks.

```bash
/ralph-beads --mode plan --max-iterations 3 "Add simple feature X"
```

**Expected:**
- [ ] Creates epic with title "Proto: Add simple feature X"
- [ ] Epic has `ralph` and `template` labels
- [ ] Epic state set to `mode=planning`
- [ ] Loop continues until `<promise>PLAN_READY</promise>` or max iterations

**Verify:**
```bash
bd list --type=epic --label=ralph
bd show <epic-id>
```

### Test 3: Building Mode (New Epic)

**Purpose:** Verify building mode creates and executes molecule.

```bash
/ralph-beads --mode build --max-iterations 5 "Fix typo in README"
```

**Expected:**
- [ ] Creates epic and pours into molecule
- [ ] Epic state set to `mode=building`
- [ ] Loop continues until `<promise>DONE</promise>` or max iterations

**Verify:**
```bash
bd list --type=epic --label=ralph
bd --no-daemon mol show <mol-id>
bd --no-daemon mol progress <mol-id>
```

### Test 4: Resume Existing Molecule

**Purpose:** Verify resume functionality.

```bash
# First, get an existing molecule ID
bd list --type=molecule

# Resume it
/ralph-beads --resume <mol-id> --max-iterations 5 "Continue work"
```

**Expected:**
- [ ] Does not create new epic
- [ ] Continues from existing molecule state
- [ ] Iteration count continues from previous

### Test 5: Status Check

```bash
/ralph-status
/ralph-status --verbose
/ralph-status <epic-id>
```

**Expected:**
- [ ] Shows epic title, status, progress
- [ ] Shows task completion percentage
- [ ] Shows blocked count
- [ ] With --verbose, shows recent iteration logs

### Test 6: Cancel Workflow

```bash
# Start a workflow
/ralph-beads --mode plan "Test cancel"

# Cancel it (in same or different terminal)
/ralph-cancel --reason "Testing cancellation"
```

**Expected:**
- [ ] Epic state changes to `mode=paused`
- [ ] Comment added with cancellation reason
- [ ] Resume instructions provided

### Test 7: Complexity Detection

```bash
# Should detect TRIVIAL
/ralph-beads --dry-run "Fix typo in readme"

# Should detect SIMPLE
/ralph-beads --dry-run "Add toggle button"

# Should detect CRITICAL
/ralph-beads --dry-run "Refactor user authentication"

# Override detection
/ralph-beads --dry-run --complexity critical "Fix typo"
```

**Expected:** Correct complexity and iteration counts shown.

### Test 8: Worktree Isolation

```bash
# Requires building mode with molecule
/ralph-beads --worktree --mode build --epic <epic-id> "Work in isolation"
```

**Expected:**
- [ ] Creates git worktree at `../worktree-<mol-id>`
- [ ] Creates branch `molecule/<mol-id>`
- [ ] Instructions to cd into worktree shown

**Cleanup:**
```bash
git worktree remove ../worktree-<mol-id>
```

## OpenCode Plugin Testing

### Prerequisites

```bash
cd ~/code/ralph-beads/.opencode

# Install dependencies
bun install

# Build plugin (if TypeScript)
bun run build  # or: bunx tsc
```

### Test 1: Plugin Loads in OpenCode

**Purpose:** Verify plugin registers without errors.

1. Start OpenCode with plugin path configured
2. Check console/logs for: `asz: RalphBeads plugin loaded!`

**Expected:** No errors, plugin loaded message appears.

### Test 2: Tool Registration

**Purpose:** Verify tools appear in OpenCode.

In OpenCode, check available tools or type:
```
/help
```

**Expected:**
- [ ] `ralph-beads` tool available
- [ ] `ralph-status` tool available
- [ ] `ralph-cancel` tool available

### Test 3: Planning Mode Creates Epic

In OpenCode session:
```
Use the ralph-beads tool with task="Test planning" and mode="plan"
```

**Expected:**
- [ ] Epic created with correct title
- [ ] State machine set to planning
- [ ] Planning prompt returned

### Test 4: Building Mode Executes Tasks

```
Use the ralph-beads tool with task="Simple fix" and mode="build"
```

**Expected:**
- [ ] Epic created and poured into molecule
- [ ] Building prompt returned
- [ ] bd ready shows tasks

### Test 5: Stop Hook Continues Loop

**Purpose:** Verify stop hook prevents early termination.

1. Start a ralph-beads workflow
2. Try to stop/exit the session without completing
3. Observe if prompt is injected to continue

**Expected:**
- [ ] Stop hook detects incomplete work
- [ ] Injects continuation prompt
- [ ] Iteration counter increments

### Test 6: Promise Detection

**Purpose:** Verify promise detection in output.

1. Start planning mode
2. Output `<promise>PLAN_READY</promise>` in response

**Expected:**
- [ ] State `promiseMade` set to "PLAN_READY"
- [ ] Stop hook allows exit
- [ ] Workflow completes gracefully

## Integration Test Scenarios

### E2E Scenario 1: Full Planning to Building Cycle

```bash
# 1. Start planning
/ralph-beads --mode plan --max-iterations 3 "Add user profile page"

# 2. Complete planning (output PLAN_READY)
# ... planning iterations ...

# 3. Check proto is ready
bd show <epic-id>  # Should show mode=ready_for_build

# 4. Start building
/ralph-beads --mode build --epic <epic-id> --max-iterations 10

# 5. Complete building (output DONE)
# ... building iterations ...

# 6. Verify completion
bd show <epic-id>  # Should be closed
bd --no-daemon mol progress <mol-id>  # Should show 100%
```

### E2E Scenario 2: Resume After Interruption

```bash
# 1. Start building
/ralph-beads --mode build --max-iterations 3 "Multi-step task"

# 2. Hit max iterations (workflow pauses)

# 3. Check state preserved
bd show <epic-id>  # mode=paused

# 4. Resume
/ralph-beads --resume <mol-id> --max-iterations 10
```

### E2E Scenario 3: Circuit Breaker

```bash
# 1. Start building with a task that will fail

# 2. Observe circuit breaker after 2 failures
bd show <task-id>  # Should show status=blocked

# 3. Verify next task is selected
bd ready --mol <mol-id>  # Should show different task
```

## Automated Test Scripts

### tests/run-tests.sh

Main test runner. Executes:
- Syntax checks (bash -n)
- Shellcheck (if available)
- Prompt lint (embedded bash validation)
- Complexity heuristic verification
- Safety verification (worktree cleanup)
- Smoke test (ralph-runner invocation)
- Snapshot tests (prompt invariants)

### tests/lint-prompt-embedded.sh

Extracts and validates bash blocks from ralph-beads.md.

### tests/verify-complexity.sh

Tests complexity detection heuristics with known inputs.

### tests/verify-safety.sh

Tests signal handling and worktree cleanup.

## Test Data Cleanup

After testing, clean up test data:

```bash
# List all ralph epics
bd list --type=epic --label=ralph

# Close test epics
bd close <epic-id> --reason "Test cleanup"

# Remove test worktrees
git worktree list
git worktree remove <path>
```

## CI Integration

For CI/CD pipelines:

```yaml
# Example GitHub Actions step
- name: Run ralph-beads tests
  run: |
    ./tests/run-tests.sh
```

## Troubleshooting Tests

### "bd: command not found"
Install beads CLI or ensure it's in PATH.

### Plugin not loading
Check `claude plugins list` or OpenCode plugin configuration.

### Loop won't stop
Ensure completion promise is output exactly: `<promise>DONE</promise>`

### Worktree already exists
```bash
git worktree remove <path> --force
```
