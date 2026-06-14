# Makefile — Convenience targets for am-renderer development
# Usage:
#   make setup       Install frontend node_modules
#   make dev         Start backend (debug) + frontend dev server
#   make dev-release Start backend (release) + frontend dev server
#   make build       Release build of all crates + frontend bundle
#   make test        Run workspace unit tests
#   make test-e2e    Run integration tests (requires running backend)

.PHONY: setup dev dev-release build test test-e2e

setup:
	npm --prefix packages/web-editor install

dev: setup
	npx concurrently --kill-others-on-fail \
		--names "backend,frontend" \
		--prefix-colors "blue.bold,green.bold" \
		"cargo run -p preview-service" \
		"npm --prefix packages/web-editor run dev"

dev-release: setup
	npx concurrently --kill-others-on-fail \
		--names "backend,frontend" \
		--prefix-colors "blue.bold,green.bold" \
		"cargo run --release -p preview-service" \
		"npm --prefix packages/web-editor run dev"

build:
	cargo build --release
	npm --prefix packages/web-editor install
	npm --prefix packages/web-editor run build

test:
	cargo test --workspace --exclude integration-tests

test-e2e:
	cargo test -p integration-tests -- --test-threads=1
