# ralph-beads-cli

Rust CLI helper for the ralph-beads plugin. Provides high-performance implementations of:

- **Complexity detection** - Analyze task descriptions to determine complexity level
- **Framework detection** - Detect test framework from project files
- **Iteration calculation** - Calculate max iterations based on mode and complexity
- **State management** - Manage session state for workflow execution

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
│   └── state.rs       # Session state management
├── Cargo.toml
└── README.md
```

## License

MIT
