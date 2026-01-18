#!/bin/bash
# safe-sync.sh - Wrapper for bd sync that prevents file deletion bug
#
# Bug: bd sync uses a worktree at .git/beads-worktrees/main that can become
# stale. When stale, bd sync will commit deletions of legitimate files.
#
# This wrapper resets the worktree before syncing to prevent data loss.

set -e

WORKTREE_PATH=".git/beads-worktrees/main"

# Check if we're in a git repo with beads
if [ ! -d ".beads" ]; then
    echo "Error: Not in a beads-enabled repository" >&2
    exit 1
fi

# Fix the worktree if it exists
if [ -d "$WORKTREE_PATH" ]; then
    echo "→ Resetting beads worktree to prevent stale state..."

    # Get current branch
    CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

    # Reset the worktree to match the current branch
    (cd "$WORKTREE_PATH" && git reset --hard "$CURRENT_BRANCH" 2>/dev/null) || {
        echo "→ Worktree reset failed, removing and recreating..."
        git worktree remove "$WORKTREE_PATH" --force 2>/dev/null || true
    }
fi

# Run the actual bd sync
echo "→ Running bd sync..."
bd sync "$@"

echo "✓ Safe sync complete"
