---
description: Deep beads-integrated Ralph loop with molecule-based workflow execution
argument-hint: "[--mode plan|build] [--epic <id>] [--mol <id>] [--priority 0-4] [--complexity <level>] [--validate|--skip-validate] [--worktree] [--pr] [--max-iterations N] [--dry-run] <task>"
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
- `--complexity <trivial|simple|standard|critical>` - Override auto-detected complexity
- `--validate` - Force validation even for TRIVIAL/SIMPLE tasks
- `--skip-validate` - Skip validation for STANDARD tasks (CRITICAL cannot skip)
- `--worktree` - Execute in isolated git worktree for safe parallel execution
- `--pr` - Create PR on completion (implies --worktree)
- `--max-iterations <n>` - Maximum iterations (scaled by complexity if not specified)
- `--dry-run` - Preview setup without executing
- Everything else is the task description

## Setup Phase

### Step 1: Environment Check

```bash
# Verify beads is ready (MUST succeed to continue)
bd info || { echo "ERROR: Beads not initialized. Run 'bd init' first."; exit 1; }

# Check if daemon running (recommended but not required)
bd daemon status || echo "Consider: bd daemon start (faster graph ops)"
```

**If `bd info` fails:** Stop immediately. User must initialize beads with `bd init`.

### Step 2: Determine Mode & Defaults

**PLANNING mode** (`--mode plan`):
- Completion promise: `PLAN_READY`
- Default max-iterations: 5 (scaled by complexity)
- Goal: Create proto (template epic) with sequenced tasks

**BUILDING mode** (`--mode build` or default):
- Completion promise: `DONE`
- Default max-iterations: 20 (scaled by complexity)
- Goal: Execute molecule until complete

**Complexity-Based Scaling (applied in Step 5):**
| Complexity | Plan Iter | Build Iter |
|------------|-----------|------------|
| TRIVIAL | 2 | 5 |
| SIMPLE | 3 | 10 |
| STANDARD | 5 | 20 |
| CRITICAL | 8 | 40 |

Explicit `--max-iterations` flag always overrides scaled defaults.

### Step 3: Epic/Molecule Management

**If `--mol <id>` provided (resume molecule):**

```bash
# Note: mol commands require --no-daemon for direct DB access
# Verify molecule exists (MUST succeed)
bd --no-daemon mol show <id> || { echo "ERROR: Molecule <id> not found"; exit 1; }

bd --no-daemon mol progress <id>       # Check current progress
bd --no-daemon mol current <id>        # Show current position

# Get epic from molecule (validate JSON extraction)
EPIC_ID=$(bd --no-daemon mol show <id> --json | jq -r '.proto_id // .epic_id // empty')
[ -z "$EPIC_ID" ] && { echo "ERROR: Cannot determine epic from molecule"; exit 1; }

bd set-state $EPIC_ID mode=building
```

**If molecule not found:** Stop immediately. Check ID is correct with `bd list --type=molecule`.

**If `--epic <id>` provided (resume or pour):**

```bash
# Verify epic exists (MUST succeed)
bd show <id> || { echo "ERROR: Epic <id> not found"; exit 1; }

# For building mode, pour into molecule:
bd set-state <id> mode=building

# Pour epic into molecule (capture output)
MOL_ID=$(bd --no-daemon mol pour <id>)
[ -z "$MOL_ID" ] && { echo "ERROR: Failed to pour epic into molecule"; exit 1; }
echo "Created molecule: $MOL_ID"
```

**If epic not found:** Verify ID with `bd list --type=epic`.

**Otherwise (new epic/proto):**

Detect epic type from task keywords:
- "fix", "bug", "error", "crash" → type=bug
- "feat", "add", "implement", "create" → type=feature
- Default → type=task

```bash
# Create proto (template for molecule) - capture ID
EPIC_ID=$(bd create --type=epic --title="Proto: <task-summary>" --priority=<priority> --json | jq -r '.id // empty')
[ -z "$EPIC_ID" ] && { echo "ERROR: Failed to create epic"; exit 1; }
echo "Created epic: $EPIC_ID"

bd label add $EPIC_ID ralph
bd label add $EPIC_ID template
bd set-state $EPIC_ID mode=planning
```

**If creation fails:** Check `bd info` to verify beads is functional.

### Step 4: Auto-Detect Test Framework

```bash
# Detect framework and set test command
FRAMEWORK=""
TEST_CMD=""

if [ -f "Cargo.toml" ]; then
  FRAMEWORK="rust"
  # Prefer nextest if available
  if command -v cargo-nextest &>/dev/null; then
    TEST_CMD="cargo nextest run"
  else
    TEST_CMD="cargo test"
  fi
elif [ -f "pyproject.toml" ] || [ -f "setup.py" ]; then
  FRAMEWORK="python"
  # Prefer pytest if available
  if command -v pytest &>/dev/null || [ -f "pytest.ini" ]; then
    TEST_CMD="pytest"
  else
    TEST_CMD="python -m unittest discover"
  fi
elif [ -f "package.json" ]; then
  FRAMEWORK="node"
  # Check for test script in package.json
  if grep -q '"test"' package.json 2>/dev/null; then
    TEST_CMD="npm test"
  else
    TEST_CMD="echo 'No test script defined'"
  fi
fi

# Label the epic with detected framework
[ -n "$FRAMEWORK" ] && bd label add $EPIC_ID framework:$FRAMEWORK

# Output detected configuration
echo "Test framework: ${FRAMEWORK:-none}"
echo "Test command: ${TEST_CMD:-none}"
```

**Note:** The detected `TEST_CMD` should be used in the building prompt for running tests.

### Step 5: Auto-Detect Complexity & Scale Iterations

```bash
# Detect complexity from task description
# Default to STANDARD, override with --complexity flag if provided
COMPLEXITY="${COMPLEXITY_ARG:-standard}"

# Only auto-detect if no explicit override
if [ -z "$COMPLEXITY_ARG" ]; then
  # TRIVIAL patterns
  if echo "$TASK" | grep -qiE 'fix typo|update comment|rename|spelling|whitespace'; then
    COMPLEXITY="trivial"
  # SIMPLE patterns
  elif echo "$TASK" | grep -qiE 'add (button|toggle|flag)|remove unused|update (version|dep)'; then
    COMPLEXITY="simple"
  # CRITICAL patterns
  elif echo "$TASK" | grep -qiE 'auth|security|payment|migration|credential|token|encrypt|password'; then
    COMPLEXITY="critical"
  fi
fi

# Label epic with complexity
bd label add $EPIC_ID complexity:$COMPLEXITY

# Apply iteration scaling (only if --max-iterations not explicitly set)
if [ -z "$MAX_ITERATIONS_ARG" ]; then
  case "$COMPLEXITY" in
    trivial)
      [ "$MODE" = "plan" ] && MAX_ITERATIONS=2 || MAX_ITERATIONS=5
      ;;
    simple)
      [ "$MODE" = "plan" ] && MAX_ITERATIONS=3 || MAX_ITERATIONS=10
      ;;
    standard)
      [ "$MODE" = "plan" ] && MAX_ITERATIONS=5 || MAX_ITERATIONS=20
      ;;
    critical)
      [ "$MODE" = "plan" ] && MAX_ITERATIONS=8 || MAX_ITERATIONS=40
      ;;
  esac
else
  MAX_ITERATIONS="$MAX_ITERATIONS_ARG"
fi

# Output detected complexity and adjusted iterations
echo "Complexity: $COMPLEXITY"
echo "Max iterations: $MAX_ITERATIONS (mode=$MODE)"
```

**Complexity Scaling Table:**
| Complexity | Plan Iter | Build Iter | Validation |
|------------|-----------|------------|------------|
| TRIVIAL | 2 | 5 | Skip |
| SIMPLE | 3 | 10 | Skip |
| STANDARD | 5 | 20 | Auto-enable |
| CRITICAL | 8 | 40 | Required |

### Step 6: Dry-Run Check

**If `--dry-run` specified:**
Display: Mode, Epic ID, Molecule ID (if any), priority, type, labels, test framework, complexity, max-iterations
Output: "DRY RUN COMPLETE - no Ralph loop started"
Exit without invoking loop.

## Start Ralph Loop

**MANDATORY:** Use the Skill tool to invoke `ralph-loop:ralph-loop`:

```
--completion-promise '<promise>' --max-iterations <N> <prompt>
```

**Concrete invocation example (Planning Mode):**

```
Skill tool call:
  skill: "ralph-loop:ralph-loop"
  args: "--completion-promise 'PLAN_READY' --max-iterations 5 'PLANNING MODE: Add JWT authentication\n\nEpic: ralph-beads-xyz\nPhase: Proto creation...'"
```

**Concrete invocation example (Building Mode):**

```
Skill tool call:
  skill: "ralph-loop:ralph-loop"
  args: "--completion-promise 'DONE' --max-iterations 20 'BUILDING MODE: Add JWT authentication\n\nMolecule: ralph-beads-mol-abc\nEpic: ralph-beads-xyz\nTest Framework: node...'"
```

**Completion promise format:** Output `<promise>PLAN_READY</promise>` or `<promise>DONE</promise>` exactly as shown (XML tags included) when criteria are met.

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
bd prime || echo "bd prime unavailable, using direct queries"
```
This is your source of truth. Do NOT rely on conversation memory.

**2. Check proto state (REQUIRED - fallback if bd prime fails):**
```bash
# Verify epic still exists
bd show <epic-id> || { echo "ERROR: Epic <epic-id> not found - may have been deleted"; exit 1; }

# Review iteration history
bd comments list <epic-id>
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
# Create task with parent in single command
TASK_ID=$(bd create --type=task --priority=<1-4> --parent=<epic-id> --title="<task title>" --json | jq -r '.id // empty')
[ -z "$TASK_ID" ] && { echo "ERROR: Failed to create task"; continue; }
echo "Created task: $TASK_ID"

# Add acceptance criteria
bd update $TASK_ID --body "$(cat <<'EOF'
## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
EOF
)"

# Add dependency on previous task (for sequencing)
# Only if there's a previous task
[ -n "$PREVIOUS_TASK_ID" ] && bd dep add $TASK_ID $PREVIOUS_TASK_ID

# Track for next iteration
PREVIOUS_TASK_ID=$TASK_ID
```

**Task structure:**
- Title: Clear, actionable (e.g., "Implement validation logic")
- Description: Acceptance criteria as checklist
- Priority: P1 = critical path, P4 = nice-to-have
- Dependencies: Form a DAG (directed acyclic graph)

## Progress Logging

At END of EVERY iteration:

```bash
bd comments add <epic-id> "[plan:N] Created T tasks. Gaps: <summary>. Next: <what to analyze>"
```

## Completion Criteria

Output `<promise>PLAN_READY</promise>` ONLY when:
- Gap analysis complete
- All tasks created with acceptance criteria
- Dependencies form valid DAG (verified below)
- `bd list --parent=<epic-id>` shows complete structure

**Validate dependency graph before completing:**
```bash
# Check for cycles or invalid structure
bd graph <epic-id>

# If cycles detected, fix with:
# bd dep remove <task-id> <problematic-dep-id>

# Verify task count and structure
bd list --parent=<epic-id>
```

Then:
```bash
# Use state machine (not labels) for workflow status
bd set-state <epic-id> mode=ready_for_build
bd comments add <epic-id> "[PROTO COMPLETE] T tasks defined, DAG validated. Pour with: /ralph-beads --mode build --epic <epic-id>"
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
bd prime || echo "bd prime unavailable, using direct queries"
```
This replaces your memory. Parse output to understand current state.

**2. Verify molecule exists:**
```bash
bd --no-daemon mol show <mol-id> || { echo "ERROR: Molecule not found"; exit 1; }
```

**3. Get next unblocked task:**
```bash
NEXT_TASK=$(bd --no-daemon ready --mol <mol-id> --limit 1 --json | jq -r '.[0].id // empty')
```
This returns the SINGLE next actionable task.
Do NOT pick a different task. Trust the algorithm.

**4. If no task returned (NEXT_TASK is empty):**
```bash
# Check completion progress
PROGRESS=$(bd --no-daemon mol progress <mol-id> --json | jq -r '.percent // 0')

if [ "$PROGRESS" = "100" ]; then
    echo "All tasks complete"
    # → output <promise>DONE</promise>
else
    # Not complete but nothing ready - check for blockers
    echo "Progress: ${PROGRESS}% - checking blockers..."
    bd list --parent=<epic-id> --status=blocked
    # Report what's blocking and why
fi
```

**5. Claim the task:**
```bash
bd update $NEXT_TASK --status=in_progress || { echo "ERROR: Failed to claim task"; exit 1; }
```

**6. Study relevant code (fresh every time):**
Use Task tool with `Explore` agent.
**DON'T ASSUME NOT IMPLEMENTED** - verify before changing.

## Circuit Breaker (CRITICAL)

**Failure tracking format:** Log each attempt with a structured comment:

```bash
# On first failure attempt
bd comments add $NEXT_TASK "[ATTEMPT:1] Failed: <error summary>"

# On second failure attempt - trigger circuit breaker
bd comments add $NEXT_TASK "[ATTEMPT:2] Failed: <error summary>. CIRCUIT BREAKER TRIGGERED."
bd update $NEXT_TASK --status=blocked
```

**Detecting previous failures:** Check comment history at iteration start:

```bash
# Count previous failure attempts on this task
ATTEMPTS=$(bd comments list $NEXT_TASK 2>/dev/null | grep -c '\[ATTEMPT:' || echo "0")

# If already 1+ failures, next failure triggers circuit breaker
if [ "$ATTEMPTS" -ge 1 ]; then
  echo "WARNING: Task has $ATTEMPTS previous failure(s). One more will block it."
fi
```

**Why this matters:** On next iteration, `bd ready` returns a DIFFERENT task.
This prevents infinite retry loops on fundamentally broken tasks.

**Important:** Use `--status=blocked`, not just a label. The status field
controls `bd ready` filtering; labels are just metadata.

**Unblocking tasks (after manual intervention):**
```bash
# See all blocked tasks
bd list --parent=<epic-id> --status=blocked

# Review what went wrong
bd comments list <blocked-task-id>

# Unblock after fixing the root cause
bd update <blocked-task-id> --status=open
bd comments add <blocked-task-id> "[UNBLOCKED] Fixed: <what was changed>"
```

## Ephemeral Tasks vs Wisps (Important Distinction)

**Two different tools for discovered work:**

| Concept | When to Use | Command |
|---------|-------------|---------|
| **Ephemeral Task** | Quick standalone work (1-2 minutes) | `bd create --ephemeral` |
| **Wisp** | Mini-molecule from a proto | `bd mol wisp <proto-id>` |

### Ephemeral Tasks

For small cleanup discovered during building (e.g., "update .gitignore"):

```bash
# Create ephemeral task (not synced to git)
TASK_ID=$(bd create --ephemeral --title="Update .gitignore" --json | jq -r '.id')
# Do the work
# Close immediately
bd close $TASK_ID
# Continue with main task
```

Ephemeral tasks:
- Create audit trail without cluttering synced backlog
- Do NOT appear in `bd ready` (you handle them immediately)
- Are local-only (not pushed to remote)

### Wisps

For substantial discovered work that needs its own molecule context:

```bash
# Create wisp from proto template
WISP_ID=$(bd mol wisp <proto-id> --title="Refactor helper module")
# Work within wisp context
bd ready --mol $WISP_ID
# Complete and squash
bd mol squash $WISP_ID
```

Wisps:
- Are ephemeral molecules with full task tracking
- Inherit structure from a proto template
- Can be burned without trace: `bd mol burn <wisp-id>`

**Decision tree:** If it takes < 5 minutes → ephemeral task. If it needs multiple steps → wisp.

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
   bd close $NEXT_TASK || echo "WARNING: Failed to close task"
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
bd comments add <mol-id> "[iter:N] [task:<task-id>] [status:<done|blocked|wip>] [tests:P/F/S] <summary>"
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
bd comments add <mol-id> "[PAUSED after N iterations] Progress: T%. Resume: /ralph-beads --mol <mol-id> --max-iterations 40"
```

## Troubleshooting: No Tasks Available

If `bd ready` returns empty but progress < 100%, diagnose the issue:

**1. Check for blocked tasks:**
```bash
bd list --parent=<epic-id> --status=blocked
```
If found: Review blocker reasons in comments, consider unblocking manually.

**2. Check for dependency cycles:**
```bash
bd graph <epic-id> | grep -i cycle
```
If found: Remove cyclic dependency with `bd dep remove`.

**3. Check for missing tasks:**
```bash
bd list --parent=<epic-id> --status=open
```
Compare count with expected. Tasks may have been accidentally closed.

**4. Check molecule sync:**
```bash
bd --no-daemon mol show <mol-id>
```
Verify molecule is linked to correct proto/epic.

**Common fixes:**
- Unblock task: `bd update <task-id> --status=open`
- Remove bad dependency: `bd dep remove <task-id> <blocks-id>`
- Reopen closed task: `bd reopen <task-id>`
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
