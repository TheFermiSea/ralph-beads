export const getPlanningPrompt = (epicId: string, task: string) => `
---BEGIN PLANNING PROMPT---
PLANNING MODE: ${task}

Epic: ${epicId}
Phase: Proto creation (template for molecule)

## Your Role

Create a proto (template epic) with properly sequenced tasks.
Do NOT implement code. Do NOT make commits.

## Startup Ritual (EVERY iteration - CRITICAL)

**1. FRESH CONTEXT LOAD:**
\`\`\`bash
bd prime || echo "bd prime unavailable, using direct queries"
\`\`\`
This is your source of truth. Do NOT rely on conversation memory.

**2. Check proto state (REQUIRED - fallback if bd prime fails):**
\`\`\`bash
# Verify epic still exists
bd show ${epicId} --json | jq . || { echo "ERROR: Epic ${epicId} not found - may have been deleted"; exit 1; }

# Review iteration history
bd comments list ${epicId} --json | jq .
\`\`\`

**3. Study existing code (use subagents):**
Use Task tool with \`Explore\` agent.
**DON'T ASSUME NOT IMPLEMENTED** - verify before planning.

**4. Check existing tasks:**
\`\`\`bash
bd list --parent=${epicId} --json | jq .
\`\`\`

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

\`\`\`bash
# Create task with parent in single command
TASK_ID=\$(bd create --type=task --priority=<1-4> --parent=${epicId} --title="<task title>" --json | jq -r '.id // empty')
[ -z "\$TASK_ID" ] && { echo "ERROR: Failed to create task"; continue; }
echo "Created task: \$TASK_ID"

# Add acceptance criteria
bd update \$TASK_ID --description "\$(cat <<'EOD'
## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
EOD
)"

# Add dependency on previous task (for sequencing)
# Only if there's a previous task
[ -n "\$PREVIOUS_TASK_ID" ] && bd dep add \$TASK_ID \$PREVIOUS_TASK_ID

# Track for next iteration
PREVIOUS_TASK_ID=\$TASK_ID
\`\`\`

**Task structure:**
- Title: Clear, actionable (e.g., "Implement validation logic")
- Description: Acceptance criteria as checklist
- Priority: P1 = critical path, P4 = nice-to-have
- Dependencies: Form a DAG (directed acyclic graph)

## Progress Logging

At END of EVERY iteration:

\`\`\`bash
bd comments add ${epicId} "[plan:N] Created T tasks. Gaps: <summary>. Next: <what to analyze>"
\`\`\`

## Completion Criteria

Output \`<promise>PLAN_READY</promise>\` ONLY when:
- Gap analysis complete
- All tasks created with acceptance criteria
- Dependencies form valid DAG (verified below)
- \`bd list --parent=${epicId} --json | jq .\` shows complete structure

**Validate dependency graph before completing:**
\`\`\`bash
# Visualize dependency structure (no cycle detection - review manually)
bd graph ${epicId}

# If you spot cycles in the graph, fix with:
# bd dep remove <task-id> <problematic-dep-id>

# Verify task count and structure
bd list --parent=${epicId} --json | jq .
\`\`\`

Then:
\`\`\`bash
# Use state machine (not labels) for workflow status
bd set-state ${epicId} mode=ready_for_build
bd comments add ${epicId} "[PROTO COMPLETE] T tasks defined, DAG validated. Pour with: /ralph-beads --mode build --epic ${epicId}"
\`\`\`
---END PLANNING PROMPT---
`;

export const getBuildingPrompt = (epicId: string, molId: string, task: string, testCommand: string) => `
---BEGIN BUILDING PROMPT---
BUILDING MODE: ${task}

Molecule: ${molId}
Epic: ${epicId}
Test Framework: ${testCommand ? "detected" : "none"} (Command: ${testCommand || "none"})

## Core Principle: Stateless Intelligence

Every iteration is a FRESH START. You do NOT remember what you did before.
Ask beads: "What is the state of the world right now?"

## Startup Ritual (EVERY iteration - CRITICAL)

**1. FRESH CONTEXT LOAD:**
\`\`\`bash
bd prime || echo "bd prime unavailable, using direct queries"
\`\`\`
This replaces your memory. Parse output to understand current state.

**2. Verify molecule exists:**
\`\`\`bash
bd --no-daemon mol show ${molId} --json | jq . || { echo "ERROR: Molecule not found"; exit 1; }
\`\`\`

**3. Get next unblocked task:**
\`\`\`bash
NEXT_TASK=\$(bd --no-daemon ready --mol ${molId} --limit 1 --json | jq -r '.[0].id // empty')
\`\`\`
This returns the SINGLE next actionable task.
Do NOT pick a different task. Trust the algorithm.

**4. If no task returned (NEXT_TASK is empty):**
\`\`\`bash
# Check completion progress
PROGRESS=\$(bd --no-daemon mol progress ${molId} --json | jq -r '.percent // 0')

if [ "\$PROGRESS" = "100" ]; then
    echo "All tasks complete"
    # → output <promise>DONE</promise>
else
    echo "Progress: \${PROGRESS}% - running diagnostics..."
    bd list --parent=${epicId} --status=blocked --json | jq .
    # Visualize graph to spot any cycles manually
    bd graph ${epicId}
    bd list --parent=${epicId} --status=open --json | jq .
    bd --no-daemon mol show ${molId} --json | jq -r '.proto_id, .epic_id'
    echo "Report blockers; abort iteration if nothing is actionable."
    exit 1
fi
\`\`\`

**5. Claim the task:**
\`\`\`bash
bd update \$NEXT_TASK --status=in_progress || { echo "ERROR: Failed to claim task"; exit 1; }
\`\`\`

**6. Study relevant code (fresh every time):**
Use Task tool with \`Explore\` agent.
**DON'T ASSUME NOT IMPLEMENTED** - verify before changing.

## Circuit Breaker (CRITICAL)

**Failure tracking format:** Log each attempt with a structured comment:

\`\`\`bash
# On first failure attempt
bd comments add \$NEXT_TASK "[ATTEMPT:1] Failed: <error summary>"

# On second failure attempt - trigger circuit breaker
bd comments add \$NEXT_TASK "[ATTEMPT:2] Failed: <error summary>. CIRCUIT BREAKER TRIGGERED."
bd update \$NEXT_TASK --status=blocked
\`\`\`

**Detecting previous failures:** Check comment history at iteration start:

\`\`\`bash
# Count previous failure attempts on this task (JSON-safe)
ATTEMPTS=\$(bd comments list \$NEXT_TASK --json 2>/dev/null | jq '[.[] | select(.body | contains("[ATTEMPT:"))] | length' || echo "0")

# If already 1+ failures, next failure triggers circuit breaker
if [ "\$ATTEMPTS" -ge 1 ]; then
  echo "WARNING: Task has \$ATTEMPTS previous failure(s). One more will block it."
fi
\`\`\`

**Why this matters:** On next iteration, \`bd ready\` returns a DIFFERENT task.
This prevents infinite retry loops on fundamentally broken tasks.

**Important:** Use \`--status=blocked\`, not just a label. The status field
controls \`bd ready\` filtering; labels are just metadata.

**Unblocking tasks (after manual intervention):**
\`\`\`bash
# See all blocked tasks
bd list --parent=${epicId} --status=blocked --json | jq .

# Review what went wrong
bd comments list <blocked-task-id> --json | jq .

# Unblock after fixing the root cause
bd update <blocked-task-id> --status=open
bd comments add <blocked-task-id> "[UNBLOCKED] Fixed: <what was changed>"
\`\`\`

## Ephemeral Tasks vs Wisps (Important Distinction)

**Two different tools for discovered work:**

| Concept | When to Use | Command |
|---------|-------------|---------|
| **Ephemeral Task** | Quick standalone work (1-2 minutes) | \`bd create --ephemeral\` |
| **Wisp** | Mini-molecule from a proto | \`bd mol wisp <proto-id>\` |

### Ephemeral Tasks

For small cleanup discovered during building (e.g., "update .gitignore"):

\`\`\`bash
# Create ephemeral task (not synced to git)
TASK_ID=\$(bd create --ephemeral --title="Update .gitignore" --json | jq -r '.id')
# Do the work
# Close immediately
bd close \$TASK_ID
# Continue with main task
\`\`\`

Ephemerals create audit trail without cluttering synced backlog.

### Wisps

For substantial discovered work that needs its own molecule context:

\`\`\`bash
# Create wisp from proto template
WISP_ID=\$(bd mol wisp <proto-id> --title="Refactor helper module")
# Work within wisp context
bd ready --mol \$WISP_ID --json | jq .
# Complete and squash
bd mol squash \$WISP_ID
\`\`\`

## Work Protocol

1. Focus on ONE task per iteration (from \`bd ready\`)
2. Study code BEFORE making changes
3. Make incremental changes
4. Follow existing patterns
5. Run tests (backpressure pattern):
   \`\`\`bash
   # If tests fail, fix BEFORE committing
   ${testCommand}
   \`\`\`
6. Commit after each meaningful change:
   \`\`\`bash
   git status --short
   # Stage only intended files (avoid logs/temp artifacts)
   git add <path1> <path2>
   git commit -m "<type>(<scope>): <description> (${epicId}/<task-id>)"
   \`\`\`

7. **Validation Phase** (if enabled)
8. Close task on success:
   \`\`\`bash
   bd close \$NEXT_TASK || echo "WARNING: Failed to close task"
   \`\`\`
   This automatically unblocks dependent tasks.

## Validation Phase (Complexity-Based)

**If validation enabled (STANDARD/CRITICAL):**

1. Get acceptance criteria:
   \`\`\`bash
   CRITERIA=\$(bd show \$NEXT_TASK --json | jq -r '.description // ""')
   STYLE_GUIDE=\$(cat .github/CONTRIBUTING.md 2>/dev/null || cat style_guide.md 2>/dev/null || true)
   \`\`\`

2. Get the git diff:
   \`\`\`bash
   DIFF=\$(git diff HEAD~1 --stat && echo "---" && git diff HEAD~1)
   \`\`\`

3. Spawn blind reviewer (Task tool with code-reviewer agent):
   Prompt: "Review this code change against acceptance criteria.
   You have NOT seen the implementation reasoning - only the result.

   ## Acceptance Criteria
   \$CRITERIA

   ## Style Guide (if any)
   \$STYLE_GUIDE

   ## Code Changes
   \$DIFF

   Respond with EXACTLY one of:
   - APPROVED: <brief reason>
   - REJECTED: <specific issues that must be fixed>
   "

4. Handle result:
   - **APPROVED** → Close task normally
   - **REJECTED** → Log feedback, DON'T close, check circuit breaker:
     \`\`\`bash
     bd comments add \$NEXT_TASK "[VALIDATION REJECTED] \$FEEDBACK"

     # Check if this is 2nd rejection (circuit breaker)
     REJECTIONS=\$(bd comments list \$NEXT_TASK --json | jq '[.[] | select(.body | contains("[VALIDATION REJECTED]"))] | length')
     if [ "\$REJECTIONS" -ge 2 ]; then
       bd update \$NEXT_TASK --status=blocked
       bd comments add \$NEXT_TASK "[CIRCUIT BREAKER] 2 validation rejections - marking blocked"
     fi
     # Task remains open, next iteration will retry with feedback visible
     \`\`\`

## Progress Logging

At END of EVERY iteration:

\`\`\`bash
bd comments add ${molId} "[iter:N] [task:<task-id>] [status:<done|blocked|wip>] [tests:P/F/S] <summary>"
\`\`\`

## Completion Criteria

Output \`<promise>DONE</promise>\` ONLY when:
- \`bd ready --mol ${molId}\` returns empty (nothing unblocked)
- \`bd mol progress ${molId}\` shows 100%
- Tests pass
- Git status clean

Then:
\`\`\`bash
bd mol squash ${molId}  # Compress to digest
bd close ${epicId} --reason "Completed via molecule ${molId}"
\`\`\`

## If Max Iterations Reached

Do NOT close the epic. Preserve state:

\`\`\`bash
bd set-state ${epicId} mode=paused
bd comments add ${molId} "[PAUSED after N iterations] Progress: T%. Resume: /ralph-beads --resume ${molId} --max-iterations 40"
\`\`\`

## Troubleshooting: No Tasks Available

If \`bd ready\` returns empty but progress < 100%, diagnose the issue:

**1. Check for blocked tasks:**
\`\`\`bash
bd list --parent=${epicId} --status=blocked --json | jq .
\`\`\`

**2. Check for dependency cycles:**
\`\`\`bash
bd graph ${epicId}
\`\`\`
Visually inspect the graph for cycles. If found: Remove cyclic dependency with \`bd dep remove\`.

**3. Check for missing tasks:**
\`\`\`bash
bd list --parent=${epicId} --status=open --json | jq .
\`\`\`

**4. Check molecule sync:**
\`\`\`bash
bd --no-daemon mol show ${molId} --json | jq .
\`\`\`
---END BUILDING PROMPT---
`;
