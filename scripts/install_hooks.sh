#!/usr/bin/env sh
set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
git -C "$REPO_ROOT" config core.hooksPath .githooks
chmod +x "$REPO_ROOT/.githooks/post-commit" "$REPO_ROOT/.githooks/pre-push" "$REPO_ROOT/scripts/deploy_and_verify.sh"
echo "Installed Git hooks (.githooks/post-commit & .githooks/pre-push) for $REPO_ROOT"
