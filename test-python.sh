#!/usr/bin/env bash
set -e

echo "==> Rebuilding Python package..."
uv sync --reinstall-package infer-ts --all-extras

echo ""
echo "==> Running Python tests..."
uv run pytest tests/ -v
