# ralph-beads-cli

Rust CLI helper for the ralph-beads plugin. Provides high-performance implementations of:

- **Complexity detection** - Analyze task descriptions to determine complexity level
- **Framework detection** - Detect test framework from project files
- **Iteration calculation** - Calculate max iterations based on mode and complexity
- **State management** - Manage session state for workflow execution
- **Health checks** - Pre-execution diagnostics (git, beads, disk, etc.)
- **Security validation** - Command allowlist with risk assessment
- **Procedural memory** - Failure tracking and pattern detection

## Installation

### From source

```bash
cd ralph-beads-cli
cargo build --release
# Binary at: target/release/ralph-beads-cli
```

### Add to PATH

```bash
# Copy binary to a directory in your PATH
cp target/release/ralph-beads-cli ~/.local/bin/

# Or add target/release to PATH
export PATH="$PATH:$(pwd)/target/release"
```

## Usage

### Complexity Detection

Detect the complexity level of a task from its description:

```bash
# Text output
ralph-beads-cli detect-complexity --task "Fix typo in README"
# complexity=trivial

# JSON output
ralph-beads-cli detect-complexity --task "Implement user authentication" --format json
# {"complexity":"critical"}
```

**Complexity Levels:**

| Level | Description | Keywords |
|-------|-------------|----------|
| `trivial` | Typos, comments, whitespace | typo, rename, spelling, comment |
| `simple` | Toggles, flags, version bumps | button, toggle, flag, remove unused |
| `standard` | Typical features (default) | - |
| `critical` | Auth, security, payments | auth, security, payment, credential, encrypt |

### Framework Detection

Detect test framework from project directory:

```bash
# Current directory
ralph-beads-cli detect-framework --format json
# {"framework":"rust","test_command":"cargo nextest run"}

# Specific directory
ralph-beads-cli detect-framework --dir /path/to/project --format json
```

**Supported Frameworks:**

| Framework | Detection | Test Command |
|-----------|-----------|--------------|
| Rust | `Cargo.toml` | `cargo nextest run` or `cargo test` |
| Python | `pyproject.toml`, `setup.py` | `pytest` or `python -m unittest` |
| Node.js | `package.json` | `npm test` |
| Go | `go.mod` | `go test ./...` |
| Java/Gradle | `build.gradle` | `./gradlew test` |
| Java/Maven | `pom.xml` | `mvn test` |

### Iteration Calculation

Calculate recommended max iterations for mode and complexity:

```bash
ralph-beads-cli calc-iterations --mode build --complexity critical --format json
# {"max_iterations":"40"}

ralph-beads-cli calc-iterations --mode plan --complexity trivial
# max_iterations=2
```

**Iteration Scaling:**

| Complexity | Planning | Building |
|------------|----------|----------|
| trivial | 2 | 5 |
| simple | 3 | 10 |
| standard | 5 | 20 |
| critical | 8 | 40 |

### State Management

Create, update, and query session state:

```bash
# Create new state
ralph-beads-cli state new \
  --session-id "my-session" \
  --mode planning \
  --epic-id "epic-123" \
  --complexity critical

# Update state field
ralph-beads-cli state update \
  --state '{"session_id":"test",...}' \
  --field iteration_count \
  --value 5

# Check if loop should continue
ralph-beads-cli state should-continue \
  --state '{"session_id":"test","mode":"building",...}'
```

### Health Checks

Run pre-execution diagnostics:

```bash
# Check current directory
ralph-beads-cli health

# Check specific directory
ralph-beads-cli health --dir /path/to/project --format json
```

**Checks performed:**

| Check | What it validates |
|-------|-------------------|
| git | Git installed and repo valid |
| beads | Beads CLI installed and initialized |
| directory | Project directory exists and writable |
| git_status | Uncommitted changes (warns if >0) |
| rust | Cargo check passes (if Cargo.toml exists) |
| node | node_modules present (if package.json exists) |
| python | Virtual env present (if pyproject.toml exists) |
| disk | Available disk space |

### Security Validation

Validate commands against security rules:

```bash
# Safe command
ralph-beads-cli validate --command "cargo test"
# ✓ Allowed: true, Risk: Safe

# Dangerous command
ralph-beads-cli validate --command "git push --force origin main"
# ✗ Allowed: false, Risk: High
# Alternative: Use git push --force-with-lease

# With project root for path validation
ralph-beads-cli validate --command "cat /etc/passwd" --project-root /my/project
# ✗ Allowed: false, Risk: Medium (path outside project)
```

**Risk Levels:**

| Level | Description |
|-------|-------------|
| `safe` | Read-only, no side effects |
| `low` | Local modifications, reversible |
| `medium` | External calls, requires caution |
| `high` | Destructive, system-wide effects |
| `blocked` | Never allowed (matches blocked pattern) |

### Procedural Memory

Track failures and workarounds:

```bash
# Record success
ralph-beads-cli memory success \
  --log-file .beads/memory.jsonl \
  --task-id task-001 \
  --description "Fixed login bug"

# Record failure
ralph-beads-cli memory failure \
  --log-file .beads/memory.jsonl \
  --task-id task-002 \
  --error "Connection timed out after 30s"

# Record workaround
ralph-beads-cli memory workaround \
  --log-file .beads/memory.jsonl \
  --task-id task-002 \
  --description "Increased timeout to 60s" \
  --original-error "timeout"

# Check failure count for task
ralph-beads-cli memory failure-count \
  --log-file .beads/memory.jsonl \
  --task-id task-002

# Get active failure patterns
ralph-beads-cli memory patterns --log-file .beads/memory.jsonl

# Compile context summary
ralph-beads-cli memory compile \
  --log-file .beads/memory.jsonl \
  --epic-id epic-123
```

**Recognized Error Patterns:**

| Pattern | Matched Terms | Suggestion |
|---------|---------------|------------|
| `timeout` | timeout, timed out | Increase timeout or check network |
| `resource_not_found` | not found | Verify path/ID exists |
| `permission_denied` | permission denied | Check file permissions |
| `compile_error` | compile + error | Fix errors before tests |
| `test_failure` | test failed, assertion | Review test expectations |

### Info

Get version and capabilities:

```bash
ralph-beads-cli info --format json
```

## Integration with TypeScript

The Rust CLI is designed to be called from TypeScript via subprocess:

```typescript
import { createClient } from "./rust-client";

const client = await createClient({ $ });

// Uses Rust if available, falls back to TypeScript
const complexity = await client.detectComplexity("Add user authentication");
console.log(complexity); // "critical"
```

The TypeScript client automatically:
1. Checks if `ralph-beads-cli` is available in PATH
2. Uses Rust for high-performance operations if available
3. Falls back to TypeScript implementations if not

## Development

### Build

```bash
cargo build          # Debug build
cargo build --release # Release build (smaller, faster)
```

### Test

```bash
cargo test           # Run all tests
cargo test -- --nocapture  # Run tests with output
```

### Format & Lint

```bash
cargo fmt            # Format code
cargo clippy         # Lint code
```

## Architecture

```
ralph-beads-cli/
├── src/
│   ├── main.rs        # CLI entry point (clap)
│   ├── complexity.rs  # Complexity detection logic
│   ├── framework.rs   # Framework detection logic
│   ├── iterations.rs  # Iteration calculation
│   ├── state.rs       # Session state management
│   ├── health.rs      # Pre-execution health checks
│   ├── security.rs    # Command allowlist & validation
│   └── memory.rs      # Procedural memory (failure tracking)
├── Cargo.toml
└── README.md
```

## License

MIT
