# Spec: Core Workflow

## Overview

Ralph-beads provides two distinct operational modes: **planning** and **building**. Each mode has specific behaviors, outputs, and state transitions managed through beads.

## Requirements

### REQ-001: Planning Mode

**Priority:** P1
**Status:** draft

When invoked with `--mode plan`, ralph-beads creates a beads epic and enters a gap analysis workflow that explores the codebase, identifies requirements, and creates structured tasks.

**Acceptance Criteria:**
- [ ] Creates a new beads epic with type=epic and ralph label
- [ ] Sets epic state dimension `mode=planning`
- [ ] Uses Explore subagents for codebase analysis (not direct file reads)
- [ ] Creates child tasks under the epic for each identified work item
- [ ] Establishes dependencies between tasks using `bd dep add`
- [ ] Logs each iteration via `bd comments add` with structured format
- [ ] Outputs `<promise>PLAN_READY</promise>` when planning complete
- [ ] Sets epic state to `mode=ready_for_build` on completion
- [ ] Respects max iterations (default: 5 for planning)

**Tests:**
- [ ] Dry run creates epic structure without execution
- [ ] Epic contains properly sequenced child tasks
- [ ] Task dependencies form valid DAG (no cycles)
- [ ] Iteration logs are queryable via `bd comments list`

---

### REQ-002: Building Mode

**Priority:** P1
**Status:** draft

When invoked with `--mode build`, ralph-beads executes tasks from an existing epic, using beads for task selection and progress tracking.

**Acceptance Criteria:**
- [ ] Accepts `--epic <id>` to continue existing epic
- [ ] Sets epic state dimension `mode=building`
- [ ] Uses `bd ready --epic=<id>` for algorithmic task selection
- [ ] Updates task status to `in_progress` before starting work
- [ ] Runs tests after each implementation (backpressure pattern)
- [ ] Creates git commits with `(epic-id/task-id)` references
- [ ] Closes tasks via `bd close` on completion
- [ ] Logs each iteration with test results `[tests:pass/fail/skip]`
- [ ] Outputs `<promise>DONE</promise>` when all tasks complete
- [ ] Closes epic when 100% complete
- [ ] Respects max iterations (default: 20 for building)

**Tests:**
- [ ] Task selection respects dependencies (blocked tasks not selected)
- [ ] Failing tests trigger fix iterations (backpressure)
- [ ] Git commits include issue references
- [ ] Epic closure only when all children closed

---

### REQ-003: Mode Transitions

**Priority:** P2
**Status:** draft

The system maintains clear state transitions between modes with audit trails.

**Acceptance Criteria:**
- [ ] Epic state machine: created → planning → ready_for_build → building → complete
- [ ] State changes recorded with `bd set-state` including reason
- [ ] Interrupted work leaves epic in paused state
- [ ] Resume from paused state restores context via `bd show`
- [ ] Cannot enter building mode without plan approval

**Tests:**
- [ ] State transitions logged in beads events
- [ ] Invalid transitions rejected (e.g., building without plan)
- [ ] Resume after interruption works correctly

---

### REQ-004: Context Recovery

**Priority:** P1
**Status:** draft

Sessions can resume work after interruption, compaction, or handoff.

**Acceptance Criteria:**
- [ ] `bd show <epic>` provides full context for resumption
- [ ] `bd comments list <epic>` shows iteration history
- [ ] `bd ready --epic=<id>` shows remaining work
- [ ] No external files needed beyond beads state
- [ ] Works after context compaction (new Claude session)

**Tests:**
- [ ] Simulated compaction followed by resume
- [ ] Handoff to different Claude instance works
- [ ] All progress preserved across sessions

---

## Dependencies

- beads CLI (`bd`) must be installed and initialized
- ralph-loop plugin for iteration control
- Git repository for commit tracking

## Notes

- Planning mode emphasizes exploration over implementation
- Building mode emphasizes execution over exploration
- Both modes use structured logging for observability
