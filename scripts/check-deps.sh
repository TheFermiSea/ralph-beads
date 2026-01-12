#!/usr/bin/env bash
# check-deps.sh - Verify ralph-beads plugin dependencies are installed

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

errors=0

check_bd() {
    if ! command -v bd >/dev/null 2>&1; then
        echo -e "${RED}[MISSING]${NC} 'bd' CLI not found (install beads CLI)"
        ((errors++)) || true
        return
    fi

    BD_VERSION=$(bd --version 2>/dev/null || true)
    if bd --no-daemon mol --help >/dev/null 2>&1; then
        if [ -n "$BD_VERSION" ]; then
            echo -e "${GREEN}[OK]${NC} bd CLI with molecule support detected ($BD_VERSION)"
        else
            echo -e "${GREEN}[OK]${NC} bd CLI with molecule support detected"
        fi
    else
        echo -e "${RED}[MISSING]${NC} 'bd' CLI lacks 'mol' support. Upgrade to the latest beads CLI."
        ((errors++)) || true
    fi
}

check_plugin() {
    local plugin=$1
    if claude plugins list 2>/dev/null | grep -q "^$plugin"; then
        echo -e "${GREEN}[OK]${NC} Plugin '$plugin' is installed"
    else
        echo -e "${RED}[MISSING]${NC} Plugin '$plugin' is not installed"
        ((errors++)) || true
    fi
}

echo "Checking ralph-beads dependencies..."
echo

check_bd
echo

check_plugin "beads"
check_plugin "ralph-loop"

echo
if [ $errors -eq 0 ]; then
    echo -e "${GREEN}All dependencies satisfied!${NC}"
    exit 0
else
    echo -e "${RED}Missing $errors required plugin(s).${NC}"
    echo "Install missing plugins with: claude plugins install <plugin>"
    exit 1
fi
