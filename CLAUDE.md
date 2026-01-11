# CLAUDE.md - Ralph-Beads Plugin

## Project Overview

Ralph-Beads is a Claude Code plugin that deeply integrates the Ralph Playbook methodology with the beads issue tracker. Unlike standard Ralph which uses file-based state management, Ralph-Beads uses beads as the **single source of truth** for all execution state.

## Architecture

### Core Philosophy

**Standard Ralph:**
```
PROMPT.md + IMPLEMENTATION_PLAN.md + git commits = state
```

**Ralph-Beads:**
```
Beads epic + child tasks + dependencies + comments = state
```

### Key Components

| Component | Purpose |
|-----------|---------|
| `commands/ralph-beads.md` | Main command with planning/building modes |
| `commands/ralph-status.md` | Epic status and progress display |
| `commands/ralph-cancel.md` | Graceful loop cancellation |
| `hooks/` | Stop hook for loop control (inherits from ralph-loop) |
| `scripts/` | Helper scripts for beads operations |

### Beads Integration Points

| Ralph Concept | Beads Implementation |
|---------------|---------------------|
| Task list | `bd create --parent=<epic>` (child tasks) |
| Task ordering | `bd dep add` (dependencies) |
| Progress tracking | `bd comments add` (iteration logs) |
| Mode switching | `bd set-state` (state dimensions) |
| Task selection | `bd ready --epic=<id>` (unblocked work) |
| Completion check | `bd epic status` (percentage complete) |
| Visualization | `bd graph` (dependency tree) |

## Development Guidelines

### Testing Changes

1. Install plugin locally:
   ```bash
   cd ~/code/ralph-beads
   claude plugins install .
   ```

2. Test with dry-run:
   ```bash
   /ralph-beads --dry-run --mode plan "Test task"
   ```

3. Test with low iterations:
   ```bash
   /ralph-beads --max-iterations 3 "Simple test task"
   ```

### Beads Commands Reference

```bash
# Epic management
bd create --type=epic --title="..." --priority=2
bd epic status <id>

# Task management
bd create --parent=<epic> --type=task --title="..."
bd dep add <task-id> <depends-on-id>
bd ready --epic=<id>

# State management
bd set-state <id> mode=planning
bd set-state <id> mode=building
bd state <id> mode

# Progress tracking
bd comments add <id> --body "..."
bd comments list <id>

# Visualization
bd graph <id>
bd activity --follow --mol <id>
```

### Code Patterns

**Creating sequenced tasks:**
```bash
# Create tasks
TASK1=$(bd q --parent=$EPIC --type=task --title="First task")
TASK2=$(bd q --parent=$EPIC --type=task --title="Second task")
TASK3=$(bd q --parent=$EPIC --type=task --title="Third task")

# Add dependencies (each depends on previous)
bd dep add $TASK2 $TASK1
bd dep add $TASK3 $TASK2
```

**Finding next task:**
```bash
# Get first unblocked task
NEXT=$(bd ready --epic=$EPIC --json | jq -r '.[0].id')
bd update $NEXT --status=in_progress
```

**Logging iteration:**
```bash
bd comments add $EPIC --body "[iter:$N] [task:$TASK] [tests:$PASS/$FAIL/$SKIP] Summary: $MSG"
```

## File Structure

```
ralph-beads/
├── .claude-plugin/
│   └── plugin.json          # Plugin manifest
├── commands/
│   ├── ralph-beads.md       # Main command
│   ├── ralph-status.md      # Status display
│   └── ralph-cancel.md      # Cancel command
├── hooks/
│   └── hooks.json           # Hook configuration (if needed)
├── scripts/
│   └── setup.sh             # Setup helpers
├── docs/
│   ├── architecture.md      # Detailed architecture
│   ├── comparison.md        # Ralph vs Ralph-Beads
│   └── examples.md          # Usage examples
├── specs/
│   └── *.md                 # Specification files (for spec-kit)
├── CLAUDE.md                # This file
├── README.md                # User documentation
└── LICENSE                  # MIT license
```

## Dependencies

This plugin depends on:
- `beads` plugin (for issue tracking)
- `ralph-loop` plugin (for loop mechanics)

## Common Issues

### "bd: command not found"
Beads CLI must be installed. See: https://github.com/steveyegge/beads

### "No epic found"
Ensure `--epic <id>` is provided for resume, or create new with `/ralph-beads --mode plan "task"`.

### Loop doesn't stop
The completion promise must be output exactly: `<promise>DONE</promise>` or `<promise>PLAN_READY</promise>`.

## Future Enhancements

- [ ] Swarm integration for parallel task execution
- [ ] Activity feed integration for real-time monitoring
- [ ] Gate support for multi-agent coordination
- [ ] Spec-kit integration for requirements management
