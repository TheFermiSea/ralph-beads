# Ralph vs Ralph-Beads: Detailed Comparison

## Executive Summary

| Aspect | Standard Ralph | Ralph-Beads |
|--------|---------------|-------------|
| State Storage | File-based (markdown) | Database-backed (beads) |
| Context Recovery | Re-read files | Query beads |
| Task Dependencies | Implicit (plan order) | Explicit (`bd dep add`) |
| Task Selection | LLM judgment | Algorithmic (`bd ready`) |
| Cross-Session | Git commits only | Full state persistence |
| Monitoring | Manual inspection | Real-time feeds |
| Multi-Agent | Not supported | Gates, swarms (future) |

## Detailed Comparison

### 1. State Management

**Standard Ralph:**
```
.claude/
├── PROMPT.md                    # Loop instructions
├── PROMPT_plan.md               # Planning mode
├── PROMPT_build.md              # Building mode
├── AGENTS.md                    # Operational guide
└── IMPLEMENTATION_PLAN.md       # Task list (markdown bullets)
```

**Ralph-Beads:**
```
.beads/
├── issues.jsonl                 # All state in one place
└── config.json                  # Configuration

# No separate files needed - beads IS the state
```

### 2. Task Representation

**Standard Ralph (IMPLEMENTATION_PLAN.md):**
```markdown
## Tasks

- [ ] P1: Implement core validation logic
  - Accept: All inputs validated before processing
  - Tests: Unit tests for validation functions
- [ ] P1: Add error handling
  - Accept: Graceful error messages
  - Tests: Error path coverage
- [ ] P2: Write integration tests
  - Accept: 80% coverage
- [x] P1: Setup project structure (done)
```

Problems:
- No explicit dependencies (order implies sequence)
- Checkbox state is fragile
- Can't query "what's ready to work"
- No audit trail of changes

**Ralph-Beads:**
```bash
# Create structured tasks
bd create --parent=$EPIC --type=task --priority=1 \
  --title="Implement core validation logic"

bd create --parent=$EPIC --type=task --priority=1 \
  --title="Add error handling"

bd create --parent=$EPIC --type=task --priority=2 \
  --title="Write integration tests"

# Add explicit dependencies
bd dep add $TASK_ERROR_HANDLING $TASK_VALIDATION
bd dep add $TASK_INTEGRATION $TASK_ERROR_HANDLING

# Query ready work
bd ready --epic=$EPIC
# Returns only: "Implement core validation logic" (others blocked)
```

### 3. Progress Tracking

**Standard Ralph:**
```markdown
<!-- In IMPLEMENTATION_PLAN.md -->
- [x] Task 1 (completed iteration 3)
- [ ] Task 2 (in progress)
```

No structured history. No timestamps. No metrics.

**Ralph-Beads:**
```bash
# Structured iteration logs
bd comments add $EPIC --body "[iter:1] [task:bd-xyz] [tests:10/0/0] Setup complete"
bd comments add $EPIC --body "[iter:2] [task:bd-abc] [tests:15/0/0] Validation done"
bd comments add $EPIC --body "[iter:3] [task:bd-def] [tests:20/2/0] Error handling - 2 failing tests"

# Query history
bd comments list $EPIC
# Returns timestamped, searchable log

# Built-in metrics
bd epic status $EPIC
# Returns: 2/5 tasks complete (40%), avg lead time: 12min
```

### 4. Task Selection

**Standard Ralph:**
```
Prompt: "Study @IMPLEMENTATION_PLAN.md and choose the most important item to address."
```

LLM judgment is subjective. May not respect implicit dependencies.

**Ralph-Beads:**
```bash
# Algorithmic selection
bd ready --epic=$EPIC --json | jq '.[0]'

# Returns first unblocked task by priority
# Dependencies are respected automatically
```

### 5. Mode Management

**Standard Ralph:**
```
# Separate prompt files
PROMPT_plan.md   →  planning mode
PROMPT_build.md  →  building mode

# Switch by changing which file is loaded
```

No audit trail of mode changes. Manual file switching.

**Ralph-Beads:**
```bash
# State dimension with audit trail
bd set-state $EPIC mode=planning --reason "Starting gap analysis"
bd set-state $EPIC mode=building --reason "Plan approved, starting implementation"

# Query current mode
bd state $EPIC mode
# Returns: "building"

# History preserved in event beads
bd show $EPIC --events
```

### 6. Recovery Scenarios

**Scenario: Session interrupted mid-task**

**Standard Ralph:**
1. Re-read IMPLEMENTATION_PLAN.md
2. Check git log for last commit
3. Manually determine what was in progress
4. Hope state is consistent

**Ralph-Beads:**
```bash
# Resume immediately
bd show $EPIC
# Shows: task bd-xyz is in_progress

bd comments list $EPIC --limit=1
# Shows: "[iter:5] Starting error handling..."

# Continue from exact point
bd ready --epic=$EPIC
# Returns remaining unblocked work
```

**Scenario: Need to hand off to another agent**

**Standard Ralph:**
- Share IMPLEMENTATION_PLAN.md file
- Share git history
- Write context notes manually

**Ralph-Beads:**
```bash
# Everything is in beads
bd show $EPIC --verbose
bd comments list $EPIC
bd graph $EPIC

# Other agent can pick up immediately
```

### 7. Visualization

**Standard Ralph:**
```bash
# Manual inspection
cat IMPLEMENTATION_PLAN.md | grep "\[x\]" | wc -l  # completed
cat IMPLEMENTATION_PLAN.md | grep "\[ \]" | wc -l  # remaining
```

**Ralph-Beads:**
```bash
# Built-in visualization
bd epic status $EPIC
# ████████░░░░░░░░░░░░ 40% (2/5 tasks)

bd graph $EPIC
# Visual dependency tree

bd activity --follow --mol $EPIC
# Real-time event feed
```

### 8. Failure Handling

**Standard Ralph:**
```markdown
<!-- Manually update plan -->
## Blockers
- Task 3 blocked: need API credentials
- Task 5 blocked: dependency on external service
```

**Ralph-Beads:**
```bash
# Structured blocking
bd set-state $TASK mode=blocked --reason "Need API credentials"
bd update $TASK --status=blocked

# Query all blocked work
bd blocked --epic=$EPIC

# Automatic unblocking when dependency closes
bd close $DEPENDENCY
# Dependent tasks automatically become ready
```

## When to Use Each

### Use Standard Ralph When:
- Simple, single-session tasks
- No complex dependencies
- Working alone
- Don't want beads dependency

### Use Ralph-Beads When:
- Multi-session work
- Complex task dependencies
- Need progress visibility
- Want audit trail
- Planning to hand off work
- Need structured metrics
