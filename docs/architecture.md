# Ralph-Beads Architecture

## Design Philosophy

Ralph-Beads follows the principle: **"Beads is the single source of truth."**

Standard Ralph maintains state across multiple systems:
1. `IMPLEMENTATION_PLAN.md` - Task list
2. Git commits - Progress markers
3. Source code - Implementation state

This creates synchronization challenges and cognitive overhead. Ralph-Beads consolidates all execution state into beads.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         RALPH-BEADS SYSTEM                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────┐  │
│  │   Claude Code    │    │   Ralph Loop     │    │    Beads     │  │
│  │                  │    │                  │    │              │  │
│  │  /ralph-beads    │───►│  Stop Hook       │    │  Epic        │  │
│  │  /ralph-status   │    │  Iteration Ctrl  │◄──►│  Tasks       │  │
│  │  /ralph-cancel   │    │  Promise Check   │    │  Deps        │  │
│  │                  │    │                  │    │  Comments    │  │
│  └────────┬─────────┘    └──────────────────┘    │  States      │  │
│           │                       ▲              └──────┬───────┘  │
│           │                       │                     │          │
│           ▼                       │                     ▼          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                        GIT REPOSITORY                         │  │
│  │                                                               │  │
│  │  Source Code ◄─────── Commits with (epic-id/task-id)         │  │
│  │                                                               │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### Planning Mode

```
1. User: /ralph-beads --mode plan "Feature X"
         │
2. Setup: bd create --type=epic --title="Ralph: Feature X"
         bd set-state <epic> mode=planning
         │
3. Loop:  ┌─────────────────────────────────────────┐
         │ Iteration N:                             │
         │ ├── bd show <epic>                       │
         │ ├── Explore codebase (subagents)         │
         │ ├── Gap analysis                         │
         │ ├── bd create --parent=<epic> tasks...   │
         │ ├── bd dep add (sequencing)              │
         │ └── bd comments add (log)                │
         └─────────────────────────────────────────┘
         │
4. Done:  bd set-state <epic> mode=ready_for_build
         Output: <promise>PLAN_READY</promise>
```

### Building Mode

```
1. User: /ralph-beads --mode build --epic <id>
         │
2. Setup: bd set-state <epic> mode=building
         │
3. Loop:  ┌─────────────────────────────────────────┐
         │ Iteration N:                             │
         │ ├── bd ready --epic=<epic> → next task   │
         │ ├── bd update <task> --status=in_progress│
         │ ├── Study code (subagents)               │
         │ ├── Implement                            │
         │ ├── Run tests (backpressure)             │
         │ ├── git commit -m "... (<epic>/<task>)"  │
         │ ├── bd close <task>                      │
         │ └── bd comments add <epic> (log)         │
         └─────────────────────────────────────────┘
         │
4. Done:  bd epic status <epic> = 100%
         bd close <epic>
         Output: <promise>DONE</promise>
```

## Component Details

### Epic Structure

```yaml
epic:
  id: bd-abc123
  type: epic
  title: "Ralph: Feature X"
  status: in_progress
  priority: 2
  labels:
    - ralph
    - automated
    - framework:rust
  state:
    mode: building
  children:
    - bd-task1 (complete)
    - bd-task2 (in_progress)
    - bd-task3 (blocked by bd-task2)
```

### Task Structure

```yaml
task:
  id: bd-task2
  type: task
  parent: bd-abc123
  title: "Implement validation logic"
  status: in_progress
  priority: 1
  description: |
    ## Acceptance Criteria
    - [ ] Input validation for all fields
    - [ ] Error messages are user-friendly
    - [ ] Edge cases handled

    ## Tests Required
    - [ ] Unit tests for validation functions
    - [ ] Integration tests for form submission
  depends_on:
    - bd-task1
  blocks:
    - bd-task3
```

### Comment Structure

```yaml
comment:
  issue: bd-abc123
  timestamp: 2024-01-15T10:30:00Z
  author: claude
  body: "[iter:3] [task:bd-task2] [tests:42/0/2] [commits:2] Completed validation logic implementation"
```

## State Machine

### Epic Mode States

```
                    ┌──────────────┐
                    │   created    │
                    └──────┬───────┘
                           │ /ralph-beads --mode plan
                           ▼
                    ┌──────────────┐
          ┌────────│   planning   │────────┐
          │        └──────────────┘        │
          │ interrupted                    │ PLAN_READY
          ▼                                ▼
   ┌──────────────┐                ┌──────────────┐
   │    paused    │                │ready_for_build│
   └──────────────┘                └──────┬───────┘
          ▲                               │ /ralph-beads --mode build
          │ interrupted                   ▼
          │                        ┌──────────────┐
          └────────────────────────│   building   │
                                   └──────┬───────┘
                                          │ DONE
                                          ▼
                                   ┌──────────────┐
                                   │   complete   │
                                   └──────────────┘
```

### Task States

```
pending ──► in_progress ──► complete
              │
              └──► blocked (dependency not satisfied)
```

## Integration Points

### With ralph-loop Plugin

Ralph-beads delegates loop control to the `ralph-loop` plugin:
- Stop hook for iteration control
- Completion promise detection
- Max iteration enforcement

### With Beads

All state operations go through beads CLI:
- Issue CRUD: `bd create`, `bd update`, `bd close`
- Dependencies: `bd dep add`, `bd ready`
- State: `bd set-state`, `bd state`
- Progress: `bd comments add`
- Queries: `bd list`, `bd epic status`

### With Git

Commits include issue references for traceability:
```
feat(auth): implement login validation (bd-abc123/bd-task2)
```

## Future Extensions

### Swarm Support

For parallel task execution:
```bash
bd swarm create --epic=<id> --parallel=3
```

### Gate Support

For multi-agent coordination:
```bash
bd gate create <epic> --type=approval --required=2
```

### Activity Feed

For real-time monitoring:
```bash
bd activity --follow --mol <epic>
```
