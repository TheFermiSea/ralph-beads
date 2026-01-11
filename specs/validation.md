# Spec: Blind Validation

## Overview

Blind validation spawns an independent reviewer after task completion who only sees acceptance criteria and code diff, not the implementation reasoning. This prevents confirmation bias and ensures code changes meet stated requirements.

## Requirements

### REQ-040: Complexity-Based Validation Trigger

**Priority:** P1
**Status:** draft

Validation automatically triggers based on detected task complexity. Higher complexity tasks require more rigorous validation.

**Complexity Detection:**
- TRIVIAL: fix typo, update comment, rename, spelling, whitespace
- SIMPLE: add button/toggle/flag, remove unused, update version/dep
- STANDARD: default for unmatched patterns
- CRITICAL: auth, security, payment, migration, credential, token, encrypt, password

**Scaling Table:**
| Complexity | Plan Iter | Build Iter | Validation |
|------------|-----------|------------|------------|
| TRIVIAL | 2 | 5 | Skip |
| SIMPLE | 3 | 10 | Skip |
| STANDARD | 5 | 20 | Auto-enable |
| CRITICAL | 8 | 40 | Required |

**Acceptance Criteria:**
- [ ] Complexity auto-detected from task description keywords
- [ ] --complexity flag overrides auto-detection
- [ ] --validate forces validation for any complexity level
- [ ] --skip-validate skips for STANDARD (but CRITICAL cannot be skipped)
- [ ] Epic labeled with detected complexity: `complexity:<level>`
- [ ] Iteration limits adjusted based on complexity and mode

**Tests:**
- [ ] "fix typo in README" detects as TRIVIAL
- [ ] "add auth migration" detects as CRITICAL
- [ ] --complexity simple overrides CRITICAL detection
- [ ] CRITICAL tasks cannot skip validation even with --skip-validate

---

### REQ-041: Blind Review Protocol

**Priority:** P1
**Status:** draft

The reviewer receives ONLY acceptance criteria and git diff, preventing confirmation bias from seeing implementation reasoning.

**Key Insight:** The reviewer NEVER sees:
- Worker's exploration/reasoning
- Previous failed attempts
- Implementation plan details
- Conversation context

**Acceptance Criteria:**
- [ ] Reviewer spawned via Task tool with code-reviewer subagent
- [ ] Prompt contains acceptance criteria extracted from task body
- [ ] Prompt contains git diff (stat + full diff of last commit)
- [ ] NO implementation reasoning visible to reviewer
- [ ] Reviewer outputs APPROVED or REJECTED with specific reasons
- [ ] Validation result logged to task comments

**Review Prompt Template:**
```
Review this code change against acceptance criteria.
You have NOT seen the implementation reasoning - only the result.

## Acceptance Criteria
<criteria from bd show $TASK --json | jq '.body'>

## Code Changes
<git diff HEAD~1 --stat && git diff HEAD~1>

Respond with APPROVED if all criteria met, or REJECTED with specific issues.
```

**Tests:**
- [ ] Reviewer receives only criteria and diff
- [ ] APPROVED response triggers task closure
- [ ] REJECTED response preserves task as open
- [ ] Validation spawned as independent subagent

---

### REQ-042: Rejection Handling

**Priority:** P2
**Status:** draft

Rejected tasks retry in next iteration with feedback, integrating with the existing circuit breaker pattern.

**Acceptance Criteria:**
- [ ] Rejection logged to task comments with reason: `[VALIDATION REJECTED] <feedback>`
- [ ] Task NOT closed on rejection (remains in_progress)
- [ ] Next iteration sees rejection feedback in task comments
- [ ] Circuit breaker applies: 2 rejections = mark task as blocked
- [ ] Blocked tasks require manual intervention to unblock

**Rejection Flow:**
1. Validation returns REJECTED with feedback
2. Log: `bd comments add $TASK "[VALIDATION REJECTED] $FEEDBACK"`
3. Task remains open, next iteration will see feedback
4. If 2nd rejection: `bd update $TASK --status=blocked`
5. Different task selected by `bd ready` on next iteration

**Tests:**
- [ ] First rejection logs feedback, keeps task open
- [ ] Second rejection triggers circuit breaker (status=blocked)
- [ ] Blocked tasks not returned by `bd ready`
- [ ] Unblocking restores task to ready queue

---

## Dependencies

- Task tool with code-reviewer subagent type
- beads CLI for task state management
- Git repository for diff generation

## Notes

- Blind validation is inspired by Zeroshot's multi-agent coordination
- The separation prevents the reviewer from being influenced by implementation details
- Circuit breaker prevents infinite retry loops on fundamentally broken tasks
