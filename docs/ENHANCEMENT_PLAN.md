# Ralph-Beads Enhancement Plan: Zeroshot-Inspired Features

> **Source:** Analysis of [covibes/zeroshot](https://github.com/covibes/zeroshot) multi-agent coordination engine
> **Created:** 2026-01-11

## Design Decisions (User Confirmed)

| Decision | Choice |
|----------|--------|
| **Validation** | Complexity-based (auto-enable for STANDARD/CRITICAL) |
| **Complexity** | Auto-detect with `--complexity` override |
| **Isolation** | Add worktree support now |

---

## Implementation Plan

### Feature 1: Complexity Detection & Scaling

**Goal:** Auto-detect task complexity and adjust iterations/validation accordingly.

**Files to modify:**
- `commands/ralph-beads.md` - Add complexity detection logic and new arguments

**New arguments:**
```
--complexity <trivial|simple|standard|critical>  # Override auto-detection
```

**Auto-detection logic:**
```bash
# Detect complexity from task description
COMPLEXITY="standard"  # default

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

# Allow explicit override
[ -n "$COMPLEXITY_ARG" ] && COMPLEXITY="$COMPLEXITY_ARG"
```

**Scaling table:**
| Complexity | Plan Iter | Build Iter | Validation |
|------------|-----------|------------|------------|
| TRIVIAL | 2 | 5 | Skip |
| SIMPLE | 3 | 10 | Skip |
| STANDARD | 5 | 20 | Auto-enable |
| CRITICAL | 8 | 40 | Required |

---

### Feature 2: Blind Validation Phase

**Goal:** After task completion, spawn independent reviewer who only sees acceptance criteria + diff.

**Files to modify:**
- `commands/ralph-beads.md` - Add validation step to BUILDING MODE PROMPT

**New arguments:**
```
--validate           # Force validation even for TRIVIAL/SIMPLE
--skip-validate      # Skip validation even for STANDARD/CRITICAL
```

**Validation step (in BUILDING MODE PROMPT):**

```markdown
## Validation Phase (Complexity-Based)

**When validation enabled (STANDARD/CRITICAL or --validate):**

After committing code, BEFORE closing task:

1. Get the acceptance criteria:
   ```bash
   CRITERIA=$(bd show $NEXT_TASK --json | jq -r '.body // empty')
   ```

2. Get the git diff:
   ```bash
   DIFF=$(git diff HEAD~1 --stat && git diff HEAD~1)
   ```

3. Spawn blind reviewer (Task tool with code-reviewer agent):
   ```
   Review this code change against acceptance criteria.
   You have NOT seen the implementation reasoning - only the result.

   ## Acceptance Criteria
   $CRITERIA

   ## Code Changes
   $DIFF

   Respond with APPROVED if all criteria met, or REJECTED with specific issues.
   ```

4. Handle result:
   - APPROVED → close task normally
   - REJECTED → log feedback, DON'T close, mark for retry:
     ```bash
     bd comments add $NEXT_TASK "[VALIDATION REJECTED] $FEEDBACK"
     # Task remains open, next iteration will retry
     ```
```

**Key insight:** The reviewer NEVER sees:
- Worker's exploration/reasoning
- Previous failed attempts
- Implementation plan

This prevents confirmation bias.

---

### Feature 3: Worktree Isolation

**Goal:** Execute building in isolated git worktree for safe parallel execution.

**Files to modify:**
- `commands/ralph-beads.md` - Add worktree management

**New arguments:**
```
--worktree           # Execute in isolated git worktree
--pr                 # Implies --worktree, create PR on completion
```

**Worktree management:**
```bash
## Setup Phase (with --worktree)

# Create worktree for this molecule
BRANCH_NAME="molecule/$MOL_ID"
WORKTREE_PATH="../worktree-$MOL_ID"

git worktree add "$WORKTREE_PATH" -b "$BRANCH_NAME" || {
  # Branch might exist, try checkout
  git worktree add "$WORKTREE_PATH" "$BRANCH_NAME"
}

echo "Working in: $WORKTREE_PATH"
cd "$WORKTREE_PATH"

## Completion (with --worktree)

# Return to original directory
cd -

# If --pr flag, create PR
if [ "$PR_FLAG" = "true" ]; then
  git push -u origin "$BRANCH_NAME"
  gh pr create --title "$(bd show $EPIC_ID --json | jq -r '.title')" \
               --body "Completed via ralph-beads molecule $MOL_ID"
fi

# Cleanup worktree
git worktree remove "$WORKTREE_PATH"
```

**Benefits:**
- Original branch untouched during work
- Multiple molecules can run in parallel (different terminals)
- Clean PR workflow with `--pr` flag

---

## New Spec File: specs/validation.md

Create new spec for validation feature:

```markdown
# Spec: Blind Validation

## Overview

Blind validation spawns an independent reviewer after task completion who only sees
acceptance criteria and code diff, not the implementation reasoning.

## Requirements

### REQ-040: Complexity-Based Validation Trigger

**Priority:** P1

Validation automatically triggers based on detected complexity:
- TRIVIAL: Skip
- SIMPLE: Skip
- STANDARD: Enable
- CRITICAL: Required (cannot skip)

**Acceptance Criteria:**
- [ ] Complexity auto-detected from task keywords
- [ ] --complexity flag overrides auto-detection
- [ ] --validate forces validation for any complexity
- [ ] --skip-validate skips for STANDARD (but not CRITICAL)

### REQ-041: Blind Review Protocol

**Priority:** P1

Reviewer receives ONLY acceptance criteria and git diff.

**Acceptance Criteria:**
- [ ] Reviewer spawned via Task tool with code-reviewer agent
- [ ] Prompt contains acceptance criteria from task body
- [ ] Prompt contains git diff (not full file contents)
- [ ] NO implementation reasoning visible to reviewer
- [ ] Reviewer outputs APPROVED or REJECTED with specifics

### REQ-042: Rejection Handling

**Priority:** P2

Rejected tasks retry in next iteration with feedback.

**Acceptance Criteria:**
- [ ] Rejection logged to task comments with reason
- [ ] Task NOT closed on rejection
- [ ] Next iteration sees rejection feedback
- [ ] Circuit breaker still applies (2 rejections = blocked)
```

---

## File Changes Summary

| File | Changes |
|------|---------|
| `commands/ralph-beads.md` | Add complexity detection, validation step, worktree management |
| `specs/validation.md` | New file - validation requirements |
| `specs/core-workflow.md` | Update with complexity scaling |
| `CLAUDE.md` | Document new flags |
| `README.md` | User documentation updates |

---

## Verification Plan

1. **Complexity detection:**
   ```bash
   /ralph-beads --dry-run "fix typo in README"  # Should show TRIVIAL
   /ralph-beads --dry-run "add auth migration"  # Should show CRITICAL
   /ralph-beads --dry-run --complexity simple "add auth"  # Override to SIMPLE
   ```

2. **Validation:**
   ```bash
   /ralph-beads --mode build --validate --epic <id>  # Force validation
   # After task completion, verify code-reviewer subagent spawned
   ```

3. **Worktree:**
   ```bash
   /ralph-beads --mode build --worktree --epic <id>
   # Verify: working in ../worktree-<mol-id>/
   # Verify: original branch unchanged
   ```

4. **PR workflow:**
   ```bash
   /ralph-beads --mode build --pr --epic <id>
   # Verify: PR created on completion
   ```
