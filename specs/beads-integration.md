# Spec: Beads Integration

## Overview

Ralph-beads uses beads as the **single source of truth** for all execution state. This spec defines how beads features map to ralph workflow concepts.

## Requirements

### REQ-010: Epic as Container

**Priority:** P1
**Status:** draft

Each ralph-beads session operates on a beads epic that serves as the container for all related work.

**Acceptance Criteria:**
- [ ] Epic created with `bd create --type=epic`
- [ ] Epic title follows pattern: `Ralph: <description>`
- [ ] Epic has labels: `ralph`, `automated`, optionally `framework:<name>`
- [ ] All tasks are children of the epic via `--parent=<epic-id>`
- [ ] Epic status reflects overall progress (open → in_progress → closed)

**Tests:**
- [ ] `bd list --type=epic --label=ralph` returns ralph epics
- [ ] `bd show <epic>` displays all child tasks
- [ ] Epic closure blocked if children still open

---

### REQ-011: Task Selection via `bd ready`

**Priority:** P1
**Status:** draft

Task selection is algorithmic, not LLM-judgment-based. The `bd ready` command returns tasks whose dependencies are satisfied.

**Acceptance Criteria:**
- [ ] Uses `bd ready --epic=<id>` to get unblocked tasks
- [ ] Tasks returned in priority order (P0 before P1, etc.)
- [ ] Blocked tasks never returned by `bd ready`
- [ ] Empty result means all tasks complete or blocked
- [ ] First returned task is the next to work on

**Tests:**
- [ ] Task with unsatisfied dependency not in ready list
- [ ] Closing dependency makes blocked task ready
- [ ] Priority ordering verified with mixed P1/P2 tasks

---

### REQ-012: Dependency Management

**Priority:** P1
**Status:** draft

Task dependencies are explicit and enforced, replacing implicit ordering from markdown lists.

**Acceptance Criteria:**
- [ ] Dependencies added via `bd dep add <task> <depends-on>`
- [ ] `bd show <task>` displays `depends_on` and `blocks` relationships
- [ ] Circular dependencies detected and rejected
- [ ] Dependency graph visualizable via `bd graph <epic>`
- [ ] Closing a task updates blocked tasks' ready status

**Tests:**
- [ ] Dependency chain: A → B → C works correctly
- [ ] Circular dep A → B → A rejected
- [ ] `bd blocked --epic=<id>` shows blocked tasks with reasons

---

### REQ-013: Iteration Logging

**Priority:** P2
**Status:** draft

Each iteration is logged as a structured comment on the epic for observability and recovery.

**Acceptance Criteria:**
- [ ] Log format: `[iter:N] [task:id] [tests:P/F/S] [commits:N] Summary`
- [ ] Logs added via `bd comments add <epic> --body "..."`
- [ ] Logs queryable via `bd comments list <epic>`
- [ ] Logs include timestamp (automatic from beads)
- [ ] Logs survive context compaction

**Tests:**
- [ ] Comment parsing extracts iteration number
- [ ] Test results parseable from log format
- [ ] 10+ iterations logged and retrievable

---

### REQ-014: State Dimensions

**Priority:** P2
**Status:** draft

Epic-level state dimensions track mode and other workflow metadata.

**Acceptance Criteria:**
- [ ] Mode stored via `bd set-state <epic> mode=<value>`
- [ ] Valid modes: `planning`, `ready_for_build`, `building`, `paused`, `complete`
- [ ] State queryable via `bd state <epic> mode`
- [ ] State changes include reason: `--reason "..."`
- [ ] State history preserved in beads events

**Tests:**
- [ ] State persists across sessions
- [ ] Invalid mode values rejected
- [ ] State change reasons appear in event log

---

### REQ-015: Progress Metrics

**Priority:** P3
**Status:** draft

Built-in progress tracking without manual calculation.

**Acceptance Criteria:**
- [ ] `bd epic status <epic>` returns completion percentage
- [ ] Shows: total tasks, completed, in_progress, blocked
- [ ] Average lead time calculated from task open→close times
- [ ] Progress bar visualization available

**Tests:**
- [ ] 2/5 tasks complete shows 40%
- [ ] Lead time calculated correctly for closed tasks
- [ ] Metrics update immediately on task state change

---

## Anti-Patterns to Avoid

1. **Don't store state in files** - No IMPLEMENTATION_PLAN.md, no local state files
2. **Don't use LLM for task selection** - Use `bd ready` algorithmic selection
3. **Don't track progress in comments** - Use task status, comments are for logs
4. **Don't duplicate beads data** - Query beads, don't cache locally

## Dependencies

- beads >= 0.9.0 (for state dimensions)
- beads initialized in project: `.beads/` directory exists
