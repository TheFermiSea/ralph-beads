---
description: Deep beads-integrated Ralph loop with epic-based task management
argument-hint: "[--mode plan|build] [--epic <id>] [--priority 0-4] [--max-iterations N] [--dry-run] <task>"
---

# Ralph-Beads: Deep Beads Integration for Iterative Development

This command implements the Ralph Playbook methodology with **beads as the single source of truth**.
No duplicate state management—beads IS the execution control plane.

## Key Differences from Standard Ralph

| Standard Ralph | Ralph-Beads |
|----------------|-------------|
| IMPLEMENTATION_PLAN.md | Beads epic with child tasks |
| Implicit task ordering | `bd dep add` for explicit sequencing |
| Notes field for progress | `bd comments` for structured logs |
| File-based mode tracking | `bd set-state` with audit trail |
| Manual task selection | `bd ready` for unblocked work |

## Arguments

Parse the following from $ARGUMENTS:
- `--mode <plan|build>` - Execution mode (default: build)
- `--epic <id>` - Resume existing beads epic (skip creation)
- `--priority <0-4>` - Epic priority (default: 2)
- `--max-iterations <n>` - Maximum iterations (default: 5 for plan, 20 for build)
- `--dry-run` - Preview setup without executing
- Everything else is the task description

## Setup Phase

### Step 1: Determine Mode & Defaults

**PLANNING mode** (`--mode plan`):
- Completion promise: `PLAN_READY`
- Default max-iterations: 5
- Goal: Create epic structure with sequenced tasks

**BUILDING mode** (`--mode build` or default):
- Completion promise: `DONE`
- Default max-iterations: 20
- Goal: Execute tasks from epic until complete

### Step 2: Epic Management

**If `--epic <id>` provided (resume mode):**

```bash
bd show <id>                           # Verify epic exists
bd set-state <id> mode=active          # Mark as active
bd update <id> --status=in_progress    # Claim work
```

Check if epic has blockers. If blocked, report and exit.
Check `bd state <id> mode` for previous mode (planning/building).
Read comments for iteration history.

**Otherwise (new epic):**

Detect epic type from task keywords:
- "fix", "bug", "error", "crash" → type=bug
- "feat", "add", "implement", "create" → type=feature
- "refactor", "clean", "improve" → type=task
- Default → type=task

```bash
bd create --type=epic --title="Ralph: <task-summary>" --priority=<priority>
bd label add <epic-id> ralph
bd label add <epic-id> automated
bd set-state <epic-id> mode=planning
```

### Step 3: Record Metadata

```bash
bd comments add <epic-id> --body "Session started at $(date -u +%Y-%m-%dT%H:%M:%SZ)"
```

### Step 4: Auto-Detect Test Framework

Check project files:
- `Cargo.toml` → Rust: `cargo nextest run` or `cargo test`
- `pyproject.toml` / `setup.py` → Python: `pytest`
- `package.json` → Node: `npm test`

Store in epic notes or as label: `bd label add <epic-id> framework:rust`

### Step 5: Validate Prerequisites

```bash
bd ready --epic=<epic-id>  # Check if any tasks are unblocked (building mode)
bd epic status <epic-id>   # Check completion percentage
```

### Step 6: Dry-Run Check

**If `--dry-run` specified:**
Display: Mode, Epic ID, priority, type, labels, test framework, task count
Output: "DRY RUN COMPLETE - no Ralph loop started"
Exit without invoking loop.

## Start Ralph Loop

**MANDATORY:** Use the Skill tool to invoke `ralph-loop:ralph-loop`:

```
--completion-promise '<promise>' --max-iterations <N> <prompt>
```

Where `<promise>` is `PLAN_READY` (planning) or `DONE` (building).

---

## PLANNING MODE PROMPT

---BEGIN PLANNING PROMPT---
PLANNING MODE: <task description>

Epic: <epic-id>
Started: <timestamp>

## Your Role

You are in PLANNING mode. Your job is to:
1. Study the codebase and requirements
2. Perform gap analysis
3. Create child tasks under the epic with proper sequencing
4. Do NOT implement code or make commits

## Startup Ritual (EVERY iteration)

1. **Check epic state:**
   ```bash
   bd show <epic-id>
   bd comments list <epic-id> --limit=5
   ```
   Parse comments for previous iteration count.

2. **Study existing code:**
   Use Task tool with `Explore` agent. DON'T ASSUME NOT IMPLEMENTED.

3. **Check existing tasks:**
   ```bash
   bd list --epic=<epic-id>
   ```

## Planning Protocol

Use parallel subagents (Task tool with Explore) to investigate:
- Codebase structure relevant to the task
- Existing implementations
- Test coverage and patterns
- Dependencies and integration points

Perform gap analysis:
- What exists?
- What's missing?
- What needs modification?

## Create Tasks

For each identified task:

```bash
# Create task as child of epic
bd create --parent=<epic-id> --type=task --title="<task title>" --priority=<1-4>

# Add acceptance criteria to description
bd edit <task-id>  # Or use bd update with --body

# Add dependency on previous task (for sequencing)
bd dep add <this-task-id> <previous-task-id>
```

**Task structure:**
- Title: Clear, actionable (e.g., "Implement validation logic")
- Description: Acceptance criteria as checklist
- Priority: P1 = do first, P4 = do last
- Dependencies: Each task depends on its predecessor

## Progress Logging

At END of EVERY iteration:

```bash
bd comments add <epic-id> --body "[plan:N] Created T tasks, identified G gaps. Summary: <what was analyzed>"
```

## Completion Criteria

Output `<promise>PLAN_READY</promise>` ONLY when:
- Gap analysis complete
- All tasks created with acceptance criteria
- Dependencies establish execution order
- `bd list --epic=<epic-id>` shows complete task structure

Then:
```bash
bd set-state <epic-id> mode=ready_for_build
bd comments add <epic-id> --body "[PLAN COMPLETE] T tasks ready. Resume with: /ralph-beads --mode build --epic <epic-id>"
```
---END PLANNING PROMPT---

---

## BUILDING MODE PROMPT

---BEGIN BUILDING PROMPT---
BUILDING MODE: <task description>

Epic: <epic-id>
Test Framework: <framework or "none">

## Your Role

Execute tasks from the epic, one per iteration, in dependency order.

## Startup Ritual (EVERY iteration)

1. **Check epic state:**
   ```bash
   bd show <epic-id>
   bd epic status <epic-id>
   bd comments list <epic-id> --limit=3
   ```

2. **Check git state:**
   ```bash
   git status
   git log -3 --oneline
   ```

3. **Find next task:**
   ```bash
   bd ready --epic=<epic-id>
   ```
   This returns unblocked tasks (dependencies satisfied).
   Pick the first one (highest priority unblocked).

4. **Claim the task:**
   ```bash
   bd update <task-id> --status=in_progress
   ```

5. **Study relevant code:**
   Use Task tool with `Explore` agent. DON'T ASSUME NOT IMPLEMENTED.

6. **Run baseline tests:**
   If tests fail, fix them BEFORE new work.

## Subagent Strategy

- Use `Explore` agent for codebase searches (context isolation)
- Use parallel subagents for independent file reads
- Use SINGLE agent for tests/build (backpressure)
- Never run tests in parallel

## Work Protocol

1. Focus on ONE task per iteration
2. Make incremental changes
3. Follow existing patterns
4. Commit after each meaningful change:
   ```bash
   git add -A
   git commit -m "<type>(<scope>): <description> (<epic-id>/<task-id>)"
   ```

5. After task complete:
   ```bash
   bd close <task-id>
   ```
   This automatically unblocks dependent tasks.

## Progress Logging

At END of EVERY iteration:

```bash
bd comments add <epic-id> --body "[iter:N] [task:<task-id>] [tests:P/F/S] [commits:M] Summary: <what was done>"
```

## Completion Criteria

Output `<promise>DONE</promise>` ONLY when:
- `bd ready --epic=<epic-id>` returns no tasks (all complete)
- `bd epic status <epic-id>` shows 100%
- Tests pass
- Git status clean

Then:
```bash
bd close <epic-id> --reason "Completed: T tasks, N iterations, M commits"
```

## If Max Iterations Reached

Do NOT close the epic. Update state:

```bash
bd set-state <epic-id> mode=blocked
bd comments add <epic-id> --body "[BLOCKED after N iterations] Progress: T/Total tasks. Blockers: <issues>. Resume: /ralph-beads --epic <epic-id> --max-iterations 40"
```
---END BUILDING PROMPT---

---

## Quick Reference

**Start planning:**
```bash
/ralph-beads --mode plan "Implement feature X"
```

**Switch to building:**
```bash
/ralph-beads --mode build --epic <id> "Execute the plan"
```

**Resume work:**
```bash
/ralph-beads --epic <id> "Continue"
```

**Check progress:**
```bash
bd epic status <id>
bd graph <id>
bd activity --follow --mol <id>
```

$ARGUMENTS
