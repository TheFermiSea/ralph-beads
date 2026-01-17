# Ralph-Beads

**Deep beads-integrated Ralph loops for AI-supervised iterative development.**

Ralph-Beads reimagines the [Ralph Playbook](https://claytonfarr.github.io/ralph-playbook/) methodology by using [beads](https://github.com/steveyegge/beads) as the single source of truth for all execution state. No more duplicate state management between markdown files and issue trackers.

## Why Ralph-Beads?

Standard Ralph uses file-based state management:
- `IMPLEMENTATION_PLAN.md` for task tracking
- Progress notes embedded in files
- Manual task selection based on "importance"
- No dependency management
- No cross-session state beyond git commits

**Ralph-Beads uses beads as the execution control plane:**
- Epic with child tasks for structured decomposition
- `bd dep add` for explicit task sequencing
- `bd ready` for intelligent task selection (unblocked work only)
- `bd comments` for structured iteration logs
- `bd set-state` for mode transitions with audit trail
- Full persistence across sessions

## 5-Minute Quickstart

**Prerequisites:** Claude Code CLI, beads, ralph-loop plugin

```bash
# 1. Install the plugin
claude plugins install /path/to/ralph-beads

# 2. Initialize beads in your project (if not already done)
cd your-project
bd init

# 3. Start the daemon (recommended for performance)
bd daemon start

# 4. Run your first ralph-beads task
/ralph-beads "Fix the login form validation bug"
```

That's it! Ralph-beads will:
1. Create an epic with `ralph` label
2. Auto-detect your test framework
3. Start a ralph-loop to implement the fix
4. Track progress in beads
5. Commit changes when tests pass

**For larger tasks, use planning mode:**
```bash
/ralph-beads --mode plan "Add user authentication with JWT"
# Agent creates structured task plan with dependencies
# Then: /ralph-beads --mode build --epic <id>
```

**Check progress anytime:**
```bash
/ralph-status              # Current epic status
bd ready                   # What's next
```

## Installation

### Prerequisites

- [Claude Code](https://claude.ai/code) CLI
- [beads](https://github.com/steveyegge/beads) issue tracker
- `ralph-loop` plugin (from claude-plugins-official)

### Install Plugin

```bash
# Clone the repository
git clone https://github.com/briansquires/ralph-beads.git
cd ralph-beads

# Verify dependencies
./scripts/check-deps.sh

# Install as Claude Code plugin
claude plugins install .
```

### Verify Dependencies

Run the dependency checker to ensure required plugins are installed:

```bash
./scripts/check-deps.sh
```

This verifies that `beads` and `ralph-loop` plugins are available.

## Usage

### Start a New Task

```bash
# Start with planning phase (recommended for complex tasks)
/ralph-beads --mode plan "Implement user authentication system"

# Start directly with building (for simple tasks)
/ralph-beads "Fix the login button alignment"
```

### Resume Work

```bash
# Resume a specific epic
/ralph-beads --epic bd-xyz "Continue implementation"

# Check status first
/ralph-status bd-xyz
```

### Monitor Progress

```bash
# Check epic status
/ralph-status <epic-id>

# Or use beads directly
bd epic status <epic-id>
bd graph <epic-id>
bd activity --follow --mol <epic-id>
```

### Cancel Loop

```bash
/ralph-cancel --epic <epic-id> --reason "Need to pivot approach"
```

## Workflow

### Typical Flow

```
1. /ralph-beads --mode plan "Feature X"
   │
   ├── Creates epic: bd-abc
   ├── Studies codebase
   ├── Creates child tasks with dependencies
   └── Outputs: PLAN_READY

2. /ralph-beads --mode build --epic bd-abc "Execute plan"
   │
   ├── Finds unblocked task: bd ready --epic bd-abc
   ├── Implements task
   ├── Runs tests (backpressure)
   ├── Commits: git commit -m "feat: ... (bd-abc/bd-xyz)"
   ├── Closes task: bd close bd-xyz
   └── Repeats until all tasks complete

3. Epic automatically closed when 100% complete
```

### Command Reference

| Command | Description |
|---------|-------------|
| `/ralph-beads` | Main command for planning/building |
| `/ralph-status <epic>` | Check epic progress and status |
| `/ralph-cancel` | Gracefully cancel active loop |

### Arguments

```
/ralph-beads [OPTIONS] <task-description>

OPTIONS:
  --mode <plan|build>    Execution mode (default: build)
  --epic <id>            Resume existing epic
  --priority <0-4>       Epic priority (default: 2)
  --max-iterations <n>   Max iterations (default: 5/20)
  --dry-run              Preview without executing
```

## Architecture

### Beads Integration

| Ralph Concept | Beads Implementation |
|---------------|---------------------|
| Task list | Child tasks under epic |
| Task ordering | Dependencies (`bd dep add`) |
| Progress | Comments with structured format |
| Mode | State dimensions (`bd set-state`) |
| Task selection | Ready work query (`bd ready`) |
| Completion | Epic status (100%) |

### State Diagram

```
                    ┌─────────────────────────────────────┐
                    │                                     │
   /ralph-beads     │     BEADS EPIC                     │
   --mode plan      │     ┌─────────────────────────┐    │
        │           │     │ mode: planning          │    │
        └──────────►│     │ status: in_progress     │    │
                    │     │                         │    │
                    │     │ CHILDREN:               │    │
                    │     │ ├── Task 1 [ready]      │    │
                    │     │ ├── Task 2 [blocked]    │    │
                    │     │ └── Task 3 [blocked]    │    │
                    │     └─────────────────────────┘    │
                    │                │                    │
                    │                │ PLAN_READY         │
                    │                ▼                    │
   /ralph-beads     │     ┌─────────────────────────┐    │
   --mode build     │     │ mode: building          │    │
        │           │     │ status: in_progress     │    │
        └──────────►│     │                         │    │
                    │     │ CHILDREN:               │    │
                    │     │ ├── Task 1 [complete]   │    │
                    │     │ ├── Task 2 [ready]      │    │
                    │     │ └── Task 3 [blocked]    │    │
                    │     └─────────────────────────┘    │
                    │                │                    │
                    │                │ DONE               │
                    │                ▼                    │
                    │     ┌─────────────────────────┐    │
                    │     │ mode: complete          │    │
                    │     │ status: closed          │    │
                    │     │                         │    │
                    │     │ All tasks: [complete]   │    │
                    │     └─────────────────────────┘    │
                    │                                     │
                    └─────────────────────────────────────┘
```

## Comparison with Standard Ralph

| Feature | Standard Ralph | Ralph-Beads |
|---------|---------------|-------------|
| State storage | Markdown files | Beads database |
| Task dependencies | None (implicit) | Explicit with `bd dep` |
| Cross-session state | Git commits only | Full beads persistence |
| Task selection | LLM chooses "important" | `bd ready` (unblocked) |
| Progress tracking | File modifications | Structured comments |
| Mode management | Separate prompt files | State dimensions |
| Visualization | Manual inspection | `bd graph`, `bd activity` |
| Multi-agent support | None | Gates, swarms (future) |

## Common Gotchas

### "bd: command not found"
Beads CLI is not installed. Install from: https://github.com/steveyegge/beads

### "No ralph epics found"
You haven't started a ralph-beads session yet. Run:
```bash
/ralph-beads --mode plan "Your task description"
```

### Loop stops but epic isn't complete
Check for blocked tasks:
```bash
bd list --parent=<epic-id> --status=blocked
bd comments list <blocked-task-id>  # See why it's blocked
```

### Tasks not appearing in `bd ready`
Tasks only appear when their dependencies are complete. Check the graph:
```bash
bd graph <epic-id>
```

### Molecule commands fail
Molecule operations require direct DB access:
```bash
bd --no-daemon mol show <mol-id>    # Note the --no-daemon flag
```

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- [Ralph Playbook](https://claytonfarr.github.io/ralph-playbook/) - Original methodology
- [beads](https://github.com/steveyegge/beads) - Issue tracker with first-class dependency support
- [Anthropic](https://anthropic.com) - Claude Code and ralph-loop plugin
