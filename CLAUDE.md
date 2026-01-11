# CLAUDE.md - Ralph-Beads Plugin

## Project Overview

Ralph-Beads is a Claude Code plugin that deeply integrates the Ralph Playbook methodology with the beads issue tracker. Unlike standard Ralph which uses file-based state management, Ralph-Beads uses beads as the **single source of truth** for all execution state.

## Architecture

### Core Philosophy: Stateless Intelligence, Stateful Graph

**Standard Ralph:**
```
PROMPT.md + IMPLEMENTATION_PLAN.md + git commits = state
Agent relies on conversation context (CONTEXT DRIFT!)
```

**Ralph-Beads:**
```
AGENT (Claude) = PROCESSOR → Treats every iteration as FRESH START
BEADS (bd)     = HEAP      → Stores absolute truth
Agent asks beads: "What is the state of the world right now?"
```

This architecture eliminates context drift—the agent doesn't need to "remember" what it did three hours ago.

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
| **Context reload** | `bd prime` (FIRST operation every iteration!) |
| Task selection | `bd ready --mol <id>` (algorithmic, not LLM judgment) |
| Workflow scope | `bd mol pour` (molecules bias context to feature) |
| Discovered work | `bd mol wisp` (ephemeral tasks) |
| Task list | `bd create --parent=<epic>` (child tasks) |
| Task ordering | `bd dep add` (dependencies form DAG) |
| Progress tracking | `bd comments add` (iteration logs) |
| Mode switching | `bd set-state` (state dimensions) |
| Completion check | `bd mol progress` (percentage complete) |
| Visualization | `bd graph` (dependency tree) |
| Performance | `bd daemon start` (keeps graph in memory) |
| Circuit breaker | After 2 failures: `bd label add <id> blocked` |

## Development Guidelines

### Spec-Driven Development

This project uses **spec-kit as a development tool** (not a runtime dependency). Specs live in `specs/` and define requirements that we implement and validate against.

**Development Workflow:**
```
1. Write/update specs in specs/*.md
2. Run /speckit.checklist specs/<file>.md → extract acceptance criteria
3. Convert criteria to beads tasks: bd create --title="..."
4. Implement using ralph-beads itself (dogfooding!)
5. Validate: re-run checklist, verify criteria pass
6. Ship when all specs satisfied
```

**Current Specs:**
- `specs/core-workflow.md` - Planning mode, building mode, state transitions
- `specs/beads-integration.md` - Epic structure, task selection, dependencies
- `specs/commands.md` - /ralph-beads, /ralph-status, /ralph-cancel

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
# CRITICAL: Context reload (FIRST operation every iteration)
bd prime                           # AI-optimized workflow context
# Note: Molecule scope via bd ready --mol, not bd prime
# Customize: place .beads/PRIME.md to override default output

# Molecule management (requires --no-daemon for direct DB access)
bd --no-daemon mol pour <proto-id>   # Instantiate proto → molecule
bd --no-daemon mol progress <id>     # Check completion %
bd --no-daemon mol current <id>      # Current position
bd --no-daemon mol squash <id>       # Compress completed mol to digest
bd --no-daemon mol burn <mol-id>     # Discard molecule without trace

# Ephemeral tasks (for discovered work, not synced to git)
bd create --ephemeral --title="Quick cleanup task"

# Task selection (algorithmic)
bd --no-daemon ready --mol <id> --limit 1  # Single next actionable task
bd ready --parent=<epic>                    # Fallback without molecule

# Epic/Proto management
bd create --type=epic --title="Proto: ..." --label=template
bd epic status <id>

# Task management
bd create --parent=<epic> --type=task --title="..."
bd dep add <task-id> <depends-on-id>

# State management
bd set-state <id> mode=planning
bd set-state <id> mode=building
bd state <id> mode

# Progress tracking
bd comments add <id> "..."
bd comments list <id>

# Circuit breaker (after 2 failures on same task)
bd comments add <id> "Stuck: <error summary>"  # Log the reason
bd update <id> --status=blocked                 # Removes from bd ready

# Performance
bd daemon start                    # Keep graph in memory
bd daemon status                   # Check daemon

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
bd comments add $EPIC "[iter:$N] [task:$TASK] [tests:$PASS/$FAIL/$SKIP] Summary: $MSG"
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

## Plugin Development Guidelines

**Reference:** https://code.claude.com/docs/en/plugins-reference

### Directory Structure (CRITICAL)

```
ralph-beads/
├── .claude-plugin/
│   └── plugin.json          ← ONLY manifest here (nothing else!)
├── commands/                ← At root level (NOT in .claude-plugin!)
│   ├── ralph-beads.md
│   ├── ralph-status.md
│   └── ralph-cancel.md
├── specs/                   ← Development specs
├── docs/                    ← Documentation
└── CLAUDE.md
```

### plugin.json Requirements

- `name`: kebab-case, no spaces
- `version`: semantic versioning (MAJOR.MINOR.PATCH)
- `commands`: relative path starting with `./`
- All paths must be relative and use forward slashes

### Command File Format

```yaml
---
description: Required - enables Skill tool invocation
argument-hint: "[--flag] <arg>"  # Optional, shown in autocomplete
allowed-tools: Bash(git:*)       # Optional, tool restrictions
---

Markdown content for Claude...
```

### Validation Checklist

- [ ] `plugin.json` is valid JSON
- [ ] Components at root level (not in `.claude-plugin/`)
- [ ] All paths relative with `./` prefix
- [ ] Command files have `description:` frontmatter
- [ ] Test with `claude --debug`

### Common Mistakes to Avoid

| Mistake | Fix |
|---------|-----|
| Commands in `.claude-plugin/` | Move to `./commands/` at root |
| Absolute paths | Use `./` relative paths |
| Missing `description:` | Add to command frontmatter |
| Invalid JSON | Validate with `claude plugin validate` |

## Future Enhancements

- [ ] Swarm integration for parallel task execution
- [ ] Activity feed integration for real-time monitoring
- [ ] Gate support for multi-agent coordination
