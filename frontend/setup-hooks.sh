#!/bin/sh
# Enable the repo's shared git hooks (pre-commit ESLint guard).
# Run once after cloning:  sh frontend/setup-hooks.sh
cd "$(git rev-parse --show-toplevel)" || exit 1
git config core.hooksPath .githooks
echo "✓ core.hooksPath set to .githooks — pre-commit ESLint guard is active."
