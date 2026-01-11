# Ralph-Beads Architecture

## Design Philosophy: Stateless Intelligence, Stateful Graph

Ralph-Beads implements a crucial architectural principle: **decouple state from context**.

```
┌─────────────────────────────────────────────────────────────┐
│  AGENT (Claude) = PROCESSOR                                 │
│                                                             │
│  - Treats every iteration as FRESH START                    │
│  - Does NOT rely on conversation history for state          │
│  - Never says "as I mentioned earlier" or "continuing..."   │
│  - Asks beads: "What is the state of the world right now?"  │
└─────────────────────────────────────────────────────────────┘
                              ↕
┌─────────────────────────────────────────────────────────────┐
│  BEADS (bd) = HEAP                                          │
│                                                             │
│  - Stores absolute truth of done/blocked/next               │
│  - Survives context compaction and session switches         │
│  - Provides token-optimized context via bd prime            │
│  - Algorithmic task selection via bd ready                  │
└─────────────────────────────────────────────────────────────┘
```

**Why this matters:** As agents work for hours, context drift is inevitable. The context window fills with compile errors, wrong turns, and chatter. The agent "forgets" the original plan. By externalizing state to beads, we eliminate drift entirely.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         RALPH-BEADS SYSTEM                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────┐  │
│  │   Claude Code    │    │   Ralph Loop     │    │    Beads     │  │
│  │                  │    │                  │    │              │  │
│  │  /ralph-beads    │───►│  Stop Hook       │    │  bd prime    │  │
│  │  /ralph-status   │    │  Iteration Ctrl  │◄──►│  bd ready    │  │
│  │  /ralph-cancel   │    │  Promise Check   │    │  bd mol      │  │
│  │                  │    │                  │    │  Molecules   │  │
│  └────────┬─────────┘    └──────────────────┘    │  Wisps       │  │
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

## The Killer Feature: bd prime

`bd prime` is not just another list command. It runs a logic engine against your dependency graph to generate a **Context-Optimized Prompt**.

### What bd prime Does

1. **Topological Sort**: Analyzes the dependency graph (DAG) of all issues
2. **Gate Evaluation**: Checks blockers. If Task B depends on Task A, and A is open, B is invisible
3. **Context Compression**: Strips irrelevant metadata (old timestamps, verbose comments)
4. **Token Optimization**: Formats output for LLM ingestion (`[bd-123]` syntax, not verbose tables)

### Why This is Critical

Without `bd prime`, an agent will often:
- Pick a task that is actually blocked by another
- Hallucinate completion because it sees the task in history
- Get overwhelmed by a 50-item list ("choice paralysis")

`bd prime` forces focus on the **single next actionable unit of work**.

## Data Flow

### Planning Mode

```
1. User: /ralph-beads --mode plan "Feature X"
         │
2. Setup: bd create --type=epic --title="Proto: Feature X" --label=template
         bd set-state <epic> mode=planning
         │
3. Loop:  ┌─────────────────────────────────────────┐
         │ Iteration N:                             │
         │ ├── bd prime                    ◄── FIRST│
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
2. Setup: MOL_ID=$(bd mol pour <epic>)   ◄── Create molecule
         bd set-state <epic> mode=building
         │
3. Loop:  ┌─────────────────────────────────────────┐
         │ Iteration N:                             │
         │ ├── bd prime                    ◄── FIRST│
         │ ├── bd ready --mol $MOL_ID → next task   │
         │ ├── bd update <task> --status=in_progress│
         │ ├── Study code (subagents)               │
         │ ├── Implement                            │
         │ ├── Run tests (backpressure)             │
         │ ├── Circuit breaker check (2 fails max)  │
         │ ├── git commit -m "... (<epic>/<task>)"  │
         │ ├── bd close <task>                      │
         │ └── bd comments add <mol> (log)          │
         └─────────────────────────────────────────┘
         │
4. Done:  bd mol progress <mol> = 100%
         bd mol squash <mol>
         Output: <promise>DONE</promise>
```

## Molecule Architecture

### Concepts

| Term | Definition |
|------|------------|
| **Proto** | Template epic with `template` label. Defines a DAG of work that can be reused. |
| **Molecule (mol)** | Instantiated work from a proto. Persistent execution state. |
| **Wisp** | Ephemeral molecule for discovered work. Auto-cleanup after completion. |
| **Pour** | Instantiate proto → molecule (`bd mol pour <proto>`) |
| **Squash** | Compress completed molecule to digest (`bd mol squash <mol>`) |

### Why Molecules?

Without molecules, an agent might:
- Wander off to unrelated issues in the beads database
- Lose focus on the current feature branch context
- Mix tasks from multiple unrelated epics

With molecules:
- `bd ready --mol <id>` returns only tasks within the molecule's scope
- `bd prime` provides global workflow context (molecule scope via `bd ready --mol`)
- The agent stays focused on completing one coherent unit of work

### Molecule Lifecycle

```
Proto (template)
     │ bd mol pour
     ▼
Molecule (executing)
     │ tasks complete
     ▼
Molecule (100%)
     │ bd mol squash
     ▼
Digest (archived)
```

## Circuit Breaker Pattern

Prevents infinite retry loops where the agent burns API credits trying to fix an unfixable bug.

### The Protocol

```
Attempt 1: Try task → Fail → Log error → Retry
Attempt 2: Try task → Fail → CIRCUIT BREAK

bd comment <task-id> "Stuck: <error summary>"
bd label add <task-id> blocked

Next iteration: bd ready returns DIFFERENT task
```

### Why 2 Attempts?

- 1 attempt: Too aggressive, sometimes transient failures
- 3+ attempts: Too lenient, wastes credits on genuinely stuck issues
- 2 attempts: Balanced—gives benefit of doubt, then moves on

## Wisp Support

Sometimes the agent discovers cleanup work mid-task:
- "Need to update .gitignore before continuing"
- "This function should be extracted first"
- "Test data file is missing"

### Without Wisps (Bad)

Agent either:
- Does it implicitly and forgets to log it
- Adds a full issue that clutters the backlog
- Skips it and creates technical debt

### With Wisps (Good)

```bash
WISP=$(bd mol wisp "Update .gitignore")
# Do the work
bd close $WISP
# Continue with main task
```

Wisps:
- Create permanent audit trail (bd knows it happened)
- Don't clutter the backlog (ephemeral by default)
- Can be burned without trace if truly trivial (`bd mol burn <wisp>`)

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
          │ interrupted                   │ bd mol pour
          │                               ▼
          │                        ┌──────────────┐
          └────────────────────────│   building   │
                                   │  (molecule)  │
                                   └──────┬───────┘
                                          │ DONE
                                          │ bd mol squash
                                          ▼
                                   ┌──────────────┐
                                   │   complete   │
                                   └──────────────┘
```

### Task States

```
pending ──► in_progress ──► complete
              │
              └──► blocked (circuit breaker triggered)
```

## Performance: bd daemon

For optimal performance, run the beads daemon:

```bash
bd daemon start
```

The daemon:
- Keeps the dependency graph in memory
- Makes `bd prime` and `bd ready` nearly instantaneous
- Handles auto-sync to git in background
- Watches for external changes

Without daemon, each command re-parses the JSONL files. Still works, just slower.

## Integration Points

### With ralph-loop Plugin

Ralph-beads delegates loop control to `ralph-loop`:
- Stop hook for iteration control
- Completion promise detection (`<promise>DONE</promise>`)
- Max iteration enforcement

### With Git

Commits include issue references for traceability:
```
feat(auth): implement login validation (bd-abc123/bd-task2)
```

This creates a permanent link between code changes and the work item that drove them.

## Future Extensions

### Swarm Support

For parallel task execution across multiple agents:
```bash
bd swarm create --epic=<id> --parallel=3
```

### Gate Support

For multi-agent coordination:
```bash
bd gate create <epic> --type=human --await="approval"
bd gate create <epic> --type=gh:pr --await="merge"
```

### Activity Feed

For real-time monitoring:
```bash
bd activity --follow --mol <epic>
```
