#!/usr/bin/env bash
set -e

echo "==> Running Rust tests..."
cargo test

echo ""
echo "==> Regenerating FORMATS.md..."
cargo test -- --ignored dump_formats

echo ""
echo "==> Rebuilding Python package..."
uv sync --reinstall-package infer-ts --all-extras

echo ""
echo "==> Running Python tests..."
uv run pytest python/tests/ -v
