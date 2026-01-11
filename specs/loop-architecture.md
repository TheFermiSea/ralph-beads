# Spec: Loop Architecture

## Overview

This spec defines the core loop architecture for ralph-beads, implementing the "Stateless Intelligence, Stateful Graph" philosophy. The agent treats each iteration as a fresh start, with beads providing the single source of truth.

## Core Philosophy

```
┌─────────────────────────────────────────────────────────────┐
│  AGENT (Claude) = PROCESSOR                                 │
│  - Treats every iteration as fresh start                    │
│  - Does NOT rely on conversation context for state          │
│  - Asks beads: "What is the state of the world right now?"  │
└─────────────────────────────────────────────────────────────┘
                              ↕
┌─────────────────────────────────────────────────────────────┐
│  BEADS (bd) = HEAP                                          │
│  - Stores absolute truth of what's done/blocked/next        │
│  - Survives context compaction                              │
│  - Provides token-optimized context via bd prime            │
└─────────────────────────────────────────────────────────────┘
```

## Requirements

### REQ-030: Fresh Start Per Iteration

**Priority:** P0
**Status:** draft

Every loop iteration begins with a fresh context load from beads, not reliance on conversation history.

**Acceptance Criteria:**
- [ ] Each iteration starts with `bd prime` to load context
- [ ] Agent does not reference "what I did earlier" from memory
- [ ] Task state comes from `bd ready`, not agent recall
- [ ] Works correctly even after context compaction
- [ ] Works correctly if agent is replaced mid-loop

**Anti-Patterns:**
- "As I mentioned earlier..." (relying on conversation)
- "Continuing from where I left off..." (implicit state)
- Reading IMPLEMENTATION_PLAN.md for task state

---

### REQ-031: bd prime Integration

**Priority:** P0
**Status:** draft

`bd prime` is called at the start of every iteration to provide AI-optimized workflow context.

**What bd prime does:**
1. Topological sort of dependency graph (DAG)
2. Gate evaluation (blocked tasks invisible)
3. Context compression (strips irrelevant metadata)
4. Token optimization (concise syntax like `[bd-123]`)

**Acceptance Criteria:**
- [ ] `bd prime` called as FIRST operation in each iteration
- [ ] Output parsed to understand current state
- [ ] Molecule scope achieved via `bd ready --mol <id>` (bd prime is global)
- [ ] Falls back gracefully if bd prime fails
- [ ] Custom `.beads/PRIME.md` honored if present

**Example Integration:**
```
Iteration N:
1. Run: bd prime                          # Global workflow context
2. Parse: Understand workflow state
3. Execute: bd ready --mol $MOL_ID --limit 1  # Molecule-scoped task
4. Work: Implement the task
5. Test: Verify implementation
6. Close: bd close <task-id> OR mark blocked
7. Log: bd comments add ...
8. Loop: Return to step 1
```

**Note:** `bd prime` provides global context; `bd ready --mol` filters to molecule scope.

---

### REQ-032: Circuit Breaker Pattern

**Priority:** P1
**Status:** draft

Prevents infinite retry loops by marking tasks as blocked after repeated failures.

**Acceptance Criteria:**
- [ ] Track failure count per task (in iteration context)
- [ ] After 2 consecutive failures on same task: mark blocked
- [ ] On marking blocked: `bd comments add <id> "Stuck: <error summary>"`
- [ ] On marking blocked: `bd update <id> --status=blocked`
- [ ] `bd ready` skips tasks with status=blocked automatically
- [ ] Agent moves to next unblocked task
- [ ] Blocked tasks visible in `/ralph-status` output

**Circuit Breaker Flow:**
```
Attempt 1: Try task → Fail → Log error → Retry
Attempt 2: Try task → Fail → Log error → Mark blocked
Next iteration: bd ready returns different task
```

**Tests:**
- [x] After 2 failures, task gets status=blocked
- [x] `bd ready` does not return tasks with status=blocked
- [ ] Iteration continues with next task (verified at integration)

---

### REQ-033: Molecule-Based Execution

**Priority:** P1
**Status:** draft

Use beads molecules for structured workflow execution instead of raw epics.

**Molecule Concepts:**
- **Proto**: Template epic with DAG structure
- **Molecule (mol)**: Instantiated work from proto (persistent)
- **Wisp**: Ephemeral molecule (auto-deleted after completion)

**Acceptance Criteria:**
- [ ] Planning mode creates proto (template epic)
- [ ] Building mode instantiates via `bd mol pour <proto>`
- [ ] `bd ready --mol <id>` filters to molecule's steps
- [ ] `bd mol progress <id>` shows completion status
- [ ] `bd mol current <id>` shows current position
- [ ] Molecule biases task selection to its scope

**Example Workflow:**
```bash
# Planning creates a proto
bd create --type=epic --title="Proto: Add Authentication" --label=template

# Building instantiates it
MOL_ID=$(bd mol pour <proto-id> --title="Add Authentication Sprint 1")

# Work stays focused on molecule
bd ready --mol $MOL_ID
```

---

### REQ-034: Wisp Support for Discovered Work

**Priority:** P2
**Status:** draft

Support lightweight, ephemeral tasks discovered during execution.

**Scenario:** Agent realizes it needs to "update .gitignore" before continuing main task, but this shouldn't clutter project backlog.

**Acceptance Criteria:**
- [ ] `bd create --ephemeral --title="<task>"` creates ephemeral task
- [ ] Ephemeral tasks not exported to JSONL (not synced via git)
- [ ] Task closed immediately after completion
- [ ] Ephemeral tasks create local audit trail but don't clutter synced backlog
- [ ] `bd mol burn <id>` discards without trace if needed

**Example:**
```bash
# During task execution, discover cleanup needed
bd create --ephemeral --title="Update .gitignore to exclude build artifacts"
# Do the cleanup
bd close <task-id>
# Continue with main task
```

**Note:** `bd mol wisp <proto-id>` creates ephemeral molecules from protos, not ad-hoc tasks.

---

### REQ-035: Daemon Recommendation

**Priority:** P3
**Status:** draft

Recommend running `bd daemon` for optimal performance.

**Acceptance Criteria:**
- [ ] Setup instructions include `bd daemon` start
- [ ] `/ralph-beads` checks daemon status on start
- [ ] Warning if daemon not running (performance impact)
- [ ] Works correctly even without daemon (graceful degradation)

---

## Loop State Machine

```
                    ┌─────────────────────┐
                    │   ITERATION START   │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │    bd prime         │ ◄── Load fresh context
                    │    (context load)   │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  bd ready --mol     │ ◄── Get unblocked task
                    │  (task selection)   │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │    Execute work     │
                    │    (implementation) │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │    Run tests        │
                    └──────────┬──────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
     ┌────────▼────────┐  ┌────▼────┐  ┌────────▼────────┐
     │  Tests Pass     │  │ Fail #1 │  │  Fail #2+       │
     │  bd close <id>  │  │ Retry   │  │  Mark blocked   │
     │  (success)      │  │ (retry) │  │  (circuit break)│
     └────────┬────────┘  └────┬────┘  └────────┬────────┘
              │                │                │
              └────────────────┴────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │  bd comments add    │ ◄── Log iteration
                    │  (audit trail)      │
                    └──────────┬──────────┘
                               │
              ┌────────────────┼────────────────┐
              │                                 │
     ┌────────▼────────┐               ┌────────▼────────┐
     │  More work?     │               │  All complete   │
     │  → Next iter    │               │  → Exit loop    │
     └─────────────────┘               └─────────────────┘
```

## Dependencies

- beads >= 0.9.0 with molecule support
- `bd daemon` recommended but not required
- ralph-loop for iteration control
