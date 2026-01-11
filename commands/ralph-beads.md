---
description: Deep beads-integrated Ralph loop with molecule-based workflow execution
argument-hint: "[--mode plan|build] [--epic <id>] [--mol <id>] [--priority 0-4] [--max-iterations N] [--dry-run] <task>"
---

# Ralph-Beads: Stateless Intelligence, Stateful Graph

This command implements the Ralph Playbook methodology with **beads as the single source of truth**.
The agent treats each iteration as a fresh start—beads IS the execution control plane.

## Core Philosophy

```
AGENT (Claude) = PROCESSOR → Treats every iteration as fresh
BEADS (bd)     = HEAP      → Stores absolute truth
```

The agent does NOT need to "remember" what it did three hours ago.
It only needs to ask Beads: "What is the state of the world right now?"

## Key Features

| Concept | Implementation |
|---------|----------------|
| Context reload | `bd prime` every iteration |
| Task selection | `bd ready --mol <id>` (algorithmic, not LLM judgment) |
| Workflow scope | Molecules (`bd mol`) bias context to feature |
| Discovered work | Wisps (`bd mol wisp`) for ephemeral tasks |
| Circuit breaker | Mark blocked after 2 failures |

## Arguments

Parse the following from $ARGUMENTS:
- `--mode <plan|build>` - Execution mode (default: build)
- `--epic <id>` - Resume existing beads epic (skip creation)
- `--mol <id>` - Resume existing molecule (preferred over --epic)
- `--priority <0-4>` - Epic priority (default: 2)
- `--max-iterations <n>` - Maximum iterations (default: 5 for plan, 20 for build)
- `--dry-run` - Preview setup without executing
- Everything else is the task description

## Setup Phase

### Step 1: Environment Check

```bash
# Verify beads is ready
bd info

# Check if daemon running (recommended)
bd daemon status || echo "Consider: bd daemon start (faster graph ops)"
```

### Step 2: Determine Mode & Defaults

**PLANNING mode** (`--mode plan`):
- Completion promise: `PLAN_READY`
- Default max-iterations: 5
- Goal: Create proto (template epic) with sequenced tasks

**BUILDING mode** (`--mode build` or default):
- Completion promise: `DONE`
- Default max-iterations: 20
- Goal: Execute molecule until complete

### Step 3: Epic/Molecule Management

**If `--mol <id>` provided (resume molecule):**

```bash
# Note: mol commands require --no-daemon for direct DB access
bd --no-daemon mol show <id>           # Verify molecule exists
bd --no-daemon mol progress <id>       # Check current progress
bd --no-daemon mol current <id>        # Show current position
```

**If `--epic <id>` provided (resume or pour):**

```bash
bd show <id>                           # Verify epic exists
# For building mode, pour into molecule:
MOL_ID=$(bd --no-daemon mol pour <id>)
```

**Otherwise (new epic/proto):**

Detect epic type from task keywords:
- "fix", "bug", "error", "crash" → type=bug
- "feat", "add", "implement", "create" → type=feature
- Default → type=task

```bash
# Create proto (template for molecule)
bd create --type=epic --title="Proto: <task-summary>" --priority=<priority>
bd label add <epic-id> ralph
bd label add <epic-id> template
bd set-state <epic-id> mode=planning
```

### Step 4: Auto-Detect Test Framework

```bash
# Detect from project files
[ -f "Cargo.toml" ] && echo "framework:rust"
[ -f "pyproject.toml" ] && echo "framework:python"
[ -f "package.json" ] && echo "framework:node"

bd label add <epic-id> framework:<detected>
```

### Step 5: Dry-Run Check

**If `--dry-run` specified:**
Display: Mode, Epic ID, Molecule ID (if any), priority, type, labels, test framework
Output: "DRY RUN COMPLETE - no Ralph loop started"
Exit without invoking loop.

## Start Ralph Loop

**MANDATORY:** Use the Skill tool to invoke `ralph-loop:ralph-loop`:

```
--completion-promise '<promise>' --max-iterations <N> <prompt>
```

---

## PLANNING MODE PROMPT

---BEGIN PLANNING PROMPT---
PLANNING MODE: <task description>

Epic: <epic-id>
Phase: Proto creation (template for molecule)

## Your Role

Create a proto (template epic) with properly sequenced tasks.
Do NOT implement code. Do NOT make commits.

## Startup Ritual (EVERY iteration - CRITICAL)

**1. FRESH CONTEXT LOAD:**
```bash
bd prime
```
This is your source of truth. Do NOT rely on conversation memory.
If bd prime fails, fall back to checking proto state directly (step 2).

**2. Check proto state:**
```bash
bd show <epic-id>
bd comments list <epic-id> --limit=5
```

**3. Study existing code (use subagents):**
Use Task tool with `Explore` agent.
**DON'T ASSUME NOT IMPLEMENTED** - verify before planning.

**4. Check existing tasks:**
```bash
bd list --parent=<epic-id>
```

## Planning Protocol

Use parallel subagents (Task tool with Explore) to investigate:
- Codebase structure relevant to the task
- Existing implementations
- Test coverage and patterns
- Dependencies and integration points

Perform gap analysis:
- What exists? (DO NOT recreate)
- What's missing?
- What needs modification?

## Create Tasks

For each identified task:

```bash
# Create task as child of proto
TASK_ID=$(bd q --parent=<epic-id> --type=task --title="<task title>" --priority=<1-4>)

# Add acceptance criteria
bd update $TASK_ID --body "$(cat <<'EOF'
## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
EOF
)"

# Add dependency on previous task (for sequencing)
bd dep add $TASK_ID $PREVIOUS_TASK_ID
```

**Task structure:**
- Title: Clear, actionable (e.g., "Implement validation logic")
- Description: Acceptance criteria as checklist
- Priority: P1 = critical path, P4 = nice-to-have
- Dependencies: Form a DAG (directed acyclic graph)

## Progress Logging

At END of EVERY iteration:

```bash
bd comments add <epic-id> --body "[plan:N] Created T tasks. Gaps: <summary>. Next: <what to analyze>"
```

## Completion Criteria

Output `<promise>PLAN_READY</promise>` ONLY when:
- Gap analysis complete
- All tasks created with acceptance criteria
- Dependencies form valid DAG
- `bd list --parent=<epic-id>` shows complete structure

Then:
```bash
bd label add <epic-id> ready
bd set-state <epic-id> mode=ready_for_build
bd comments add <epic-id> --body "[PROTO COMPLETE] T tasks defined. Pour with: /ralph-beads --mode build --epic <epic-id>"
```
---END PLANNING PROMPT---

---

## BUILDING MODE PROMPT

---BEGIN BUILDING PROMPT---
BUILDING MODE: <task description>

Molecule: <mol-id>
Epic: <epic-id>
Test Framework: <framework or "none">

## Core Principle: Stateless Intelligence

Every iteration is a FRESH START. You do NOT remember what you did before.
Ask beads: "What is the state of the world right now?"

## Startup Ritual (EVERY iteration - CRITICAL)

**1. FRESH CONTEXT LOAD:**
```bash
bd prime
```
This replaces your memory. If bd prime fails, fall back to `bd ready` directly.
Parse the output to understand:
- Current position in workflow
- What's complete
- What's blocked
- What's next

**2. Get next unblocked task:**
```bash
bd ready --mol <mol-id> --limit 1
```
This returns the SINGLE next actionable task.
Do NOT pick a different task. Trust the algorithm.

**3. If no task returned:** Check if complete:
```bash
bd mol progress <mol-id>
```
If 100% → output `<promise>DONE</promise>`
If blocked tasks exist → report blockers

**4. Claim the task:**
```bash
bd update <task-id> --status=in_progress
```

**5. Study relevant code (fresh every time):**
Use Task tool with `Explore` agent.
**DON'T ASSUME NOT IMPLEMENTED** - verify before changing.

## Circuit Breaker (CRITICAL)

Track failures mentally within iteration. If same task fails TWICE:

```bash
# Mark blocked, move on
bd comment <task-id> "Stuck after 2 attempts: <error summary>"
bd label add <task-id> blocked
```

On next iteration, `bd ready` will return a DIFFERENT task.
This prevents infinite retry loops.

## Ephemeral Tasks for Discovered Work

If you discover a small cleanup needed (e.g., "update .gitignore"):

```bash
# Create ephemeral task (not synced to git)
bd create --ephemeral --title="Update .gitignore"
# Do the work
# Close immediately
bd close <task-id>
# Continue with main task
```

Ephemeral tasks create audit trail without cluttering the synced backlog.
Note: `bd mol wisp <proto-id>` is for ephemeral molecules from protos, not ad-hoc tasks.

## Work Protocol

1. Focus on ONE task per iteration (from `bd ready`)
2. Study code BEFORE making changes
3. Make incremental changes
4. Follow existing patterns
5. Run tests (backpressure pattern):
   ```bash
   # If tests fail, fix BEFORE committing
   <test-command>
   ```
6. Commit after each meaningful change:
   ```bash
   git add -A
   git commit -m "<type>(<scope>): <description> (<epic-id>/<task-id>)"
   ```

7. Close task on success:
   ```bash
   bd close <task-id>
   ```
   This automatically unblocks dependent tasks.

## Subagent Strategy

- Use `Explore` agent for codebase searches (isolates context)
- Use parallel subagents for independent file reads
- Use SINGLE agent for tests/build (backpressure)
- Never run tests in parallel

## Progress Logging

At END of EVERY iteration:

```bash
bd comments add <mol-id> --body "[iter:N] [task:<task-id>] [status:<done|blocked|wip>] [tests:P/F/S] <summary>"
```

## Completion Criteria

Output `<promise>DONE</promise>` ONLY when:
- `bd ready --mol <mol-id>` returns empty (nothing unblocked)
- `bd mol progress <mol-id>` shows 100%
- Tests pass
- Git status clean

Then:
```bash
bd mol squash <mol-id>  # Compress to digest
bd close <epic-id> --reason "Completed via molecule <mol-id>"
```

## If Max Iterations Reached

Do NOT close the epic. Preserve state:

```bash
bd set-state <epic-id> mode=paused
bd comments add <mol-id> --body "[PAUSED after N iterations] Progress: T%. Resume: /ralph-beads --mol <mol-id> --max-iterations 40"
```
---END BUILDING PROMPT---

---

## Quick Reference

**Start planning:**
```bash
/ralph-beads --mode plan "Implement feature X"
```

**Pour proto into molecule and build:**
```bash
/ralph-beads --mode build --epic <proto-id>
```

**Resume molecule:**
```bash
/ralph-beads --mol <mol-id>
```

**Check progress:**
```bash
bd --no-daemon mol progress <id>   # Completion %
bd prime                           # Global context
bd --no-daemon ready --mol <id>    # Molecule-scoped tasks
bd graph <id>
```

**Handle discovered work:**
```bash
bd create --ephemeral --title="Quick fix needed"
```

$ARGUMENTS
