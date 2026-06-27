# Apollo — developer & CI entry points.
#
# Targets are intentionally thin wrappers so CI and local dev run the same
# commands. `make ci` is the canonical "is this PR green?" check.

.DEFAULT_GOAL := help

.PHONY: help fmt fmt-check clippy check test check-domain ci dev-api dev-worker

help:
	@echo "Apollo make targets:"
	@echo "  make fmt           - cargo fmt --all"
	@echo "  make fmt-check     - cargo fmt --all -- --check"
	@echo "  make clippy        - cargo clippy --all-targets -- -D warnings"
	@echo "  make check         - cargo check --all-targets"
	@echo "  make test          - cargo test --all"
	@echo "  make check-domain  - enforce the domain dependency rule"
	@echo "  make ci            - fmt-check + check + clippy + check-domain + test (fail fast)"
	@echo "  make dev-api       - cargo run --bin apollo-api"
	@echo "  make dev-worker    - cargo run --bin apollo-worker"

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets -- -D warnings

check:
	cargo check --all-targets

test:
	cargo test --all

check-domain:
	./scripts/check-domain-imports.sh

ci: fmt-check check clippy check-domain test

dev-api:
	cargo run --bin apollo-api

dev-worker:
	cargo run --bin apollo-worker
