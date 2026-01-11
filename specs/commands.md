# Spec: Slash Commands

## Overview

Ralph-beads exposes three slash commands for user interaction: `/ralph-beads` (main), `/ralph-status`, and `/ralph-cancel`.

## Requirements

### REQ-020: Main Command `/ralph-beads`

**Priority:** P1
**Status:** draft

The primary entry point for starting or resuming ralph-beads workflows.

**Arguments:**
- `--mode <plan|build>` - Required. Workflow mode.
- `--epic <id>` - Optional for plan (creates new), required for build (resumes existing).
- `--max-iterations <N>` - Optional. Override default iteration limit.
- `--dry-run` - Optional. Preview setup without executing.
- `<description>` - Optional. Natural language description of work (for new epics).

**Acceptance Criteria:**
- [ ] `--mode plan "Feature X"` creates new epic and starts planning
- [ ] `--mode build --epic bd-xxx` resumes building on existing epic
- [ ] `--mode build` without epic errors with helpful message
- [ ] `--dry-run` shows what would be created without side effects
- [ ] Default iterations: 5 for plan, 20 for build
- [ ] `--max-iterations 10` overrides default

**Tests:**
- [ ] Plan mode creates epic with correct structure
- [ ] Build mode validates epic exists
- [ ] Dry run produces no beads changes
- [ ] Iteration limits enforced

**Examples:**
```bash
# Start planning a new feature
/ralph-beads --mode plan "Add user authentication"

# Resume building on existing epic
/ralph-beads --mode build --epic bd-abc123

# Preview what would happen
/ralph-beads --mode plan "Refactor API" --dry-run

# Limit iterations
/ralph-beads --mode build --epic bd-abc123 --max-iterations 10
```

---

### REQ-021: Status Command `/ralph-status`

**Priority:** P2
**Status:** draft

Query current status of a ralph-beads epic.

**Arguments:**
- `<epic-id>` - Optional. Specific epic to query. Defaults to most recent ralph epic.
- `--verbose` - Optional. Include iteration history.

**Acceptance Criteria:**
- [ ] Shows epic title, status, mode
- [ ] Shows task completion: N/M complete (X%)
- [ ] Shows current task (if in_progress)
- [ ] Shows blocked tasks count
- [ ] `--verbose` includes recent iteration logs
- [ ] No epic ID finds most recent ralph-labeled epic

**Tests:**
- [ ] Status reflects live beads state
- [ ] Verbose mode includes comments
- [ ] Handles no ralph epics gracefully

**Example Output:**
```
Epic: bd-abc123 - Ralph: Add user authentication
Status: in_progress | Mode: building
Progress: 3/7 tasks complete (43%)
Current: bd-xyz789 - Implement login validation
Blocked: 2 tasks waiting on current

Recent iterations:
  [iter:5] [task:bd-xyz789] [tests:12/0/0] Started login validation
  [iter:4] [task:bd-xyz788] [tests:10/0/0] Completed session management
```

---

### REQ-022: Cancel Command `/ralph-cancel`

**Priority:** P2
**Status:** draft

Gracefully stop an in-progress ralph-beads workflow.

**Arguments:**
- `<epic-id>` - Optional. Epic to cancel. Defaults to current/most recent.
- `--reason <text>` - Optional. Reason for cancellation.

**Acceptance Criteria:**
- [ ] Sets epic mode to `paused`
- [ ] Logs cancellation as iteration comment
- [ ] Does not close epic (allows resume)
- [ ] Reports what was in progress
- [ ] Confirms cancellation to user

**Tests:**
- [ ] Cancelled epic can be resumed
- [ ] Cancellation reason appears in logs
- [ ] In-progress task remains in_progress (not lost)

---

### REQ-023: Argument Validation

**Priority:** P2
**Status:** draft

All commands validate arguments before execution.

**Acceptance Criteria:**
- [ ] Invalid `--mode` value shows valid options
- [ ] Non-existent `--epic` ID shows helpful error
- [ ] Missing required args shows usage
- [ ] `--help` shows full usage for each command
- [ ] Invalid combinations caught (e.g., `--mode plan --epic xxx`)

**Tests:**
- [ ] Each error case produces actionable message
- [ ] Help text is accurate and complete

---

## Command Integration

### With ralph-loop Plugin

Ralph-beads delegates iteration control to ralph-loop:
- Stop hook for checking iteration limits
- Promise detection (`<promise>DONE</promise>`)
- Background loop management

### With Beads Plugin

All state operations go through beads CLI:
- Issue CRUD
- Dependency management
- State dimensions
- Comments

## Error Handling

| Error | Message | Recovery |
|-------|---------|----------|
| beads not initialized | "Run `bd init` first" | User runs bd init |
| Epic not found | "Epic bd-xxx not found. Run `bd list --type=epic`" | User checks epic ID |
| Build without plan | "Epic bd-xxx has no tasks. Run planning first." | User runs plan mode |
| Already in progress | "Epic bd-xxx already has active work. Use /ralph-cancel first." | User cancels or resumes |
