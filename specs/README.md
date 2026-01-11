# Specifications

This directory contains specifications for developing ralph-beads. We use spec-kit as a **development tool** - end users of ralph-beads don't need to know about or interact with these specs.

## Development Workflow

```
┌─────────────────────────────────────────────────────────────┐
│                    DEVELOPMENT TIME                         │
│                                                             │
│   1. Write/update specs in this directory                   │
│   2. Run spec-kit checklist to extract acceptance criteria  │
│   3. Convert criteria to beads tasks (bd create)            │
│   4. Implement using ralph-beads itself (dogfooding!)       │
│   5. Validate against specs                                 │
│   6. Ship when all specs satisfied                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Current Specs

| Spec | Status | Description |
|------|--------|-------------|
| [loop-architecture.md](loop-architecture.md) | draft | **Core architecture**: bd prime, circuit breaker, molecules, wisps |
| [core-workflow.md](core-workflow.md) | draft | Planning mode, building mode, state transitions, recovery |
| [beads-integration.md](beads-integration.md) | draft | Epic structure, task selection, dependencies, logging |
| [commands.md](commands.md) | draft | /ralph-beads, /ralph-status, /ralph-cancel |

## Using Spec-Kit

### Generate Checklists

```bash
# Generate acceptance checklist from a spec
/speckit.checklist specs/core-workflow.md

# This outputs criteria you can convert to beads tasks
```

### Convert to Beads Tasks

```bash
# Example: Convert REQ-001 criteria to tasks
bd create --type=epic --title="Implement: REQ-001 Planning Mode"

bd create --parent=<epic> --type=task --priority=1 \
  --title="Create beads epic with type=epic and ralph label"

bd create --parent=<epic> --type=task --priority=1 \
  --title="Set epic state dimension mode=planning"

# ... continue for each criterion
```

### Validate Implementation

After implementing, re-run checklists to verify:

```bash
/speckit.checklist specs/core-workflow.md
# Manually verify each criterion passes
```

## Spec Format

Each spec follows this structure:

```markdown
# Spec: <Feature Name>

## Overview
Brief description of the feature.

## Requirements

### REQ-NNN: <Requirement Title>
**Priority:** P1/P2/P3
**Status:** draft/approved/implemented

Description of the requirement.

**Acceptance Criteria:**
- [ ] Criterion 1 (testable, specific)
- [ ] Criterion 2

**Tests:**
- [ ] Test case 1
- [ ] Test case 2

---

(repeat for each requirement)

## Dependencies
What this spec depends on.

## Notes
Additional context.
```

## Why Spec-Kit for Development?

1. **AI-assisted development** - Claude builds this plugin. Specs help Claude understand requirements precisely.

2. **Quality gates** - Checklists ensure we don't ship incomplete features.

3. **Documentation** - Specs serve as living documentation of intended behavior.

4. **No runtime cost** - End users don't need spec-kit installed.

5. **Recursive dogfooding** - We use beads to track ralph-beads development tasks. Meta!

## What's NOT in Specs

- Implementation details (how code works internally)
- Deployment instructions
- User tutorials

Those belong in `docs/` or `README.md`.
