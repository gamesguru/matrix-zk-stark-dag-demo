SHELL=/bin/bash

# --- Variables ---
CARGO = cargo
PYTHON = python3

# Load environment variables from .env file if it exists
ifneq (,$(wildcard ./.env))
    include .env
endif

# Clean quotes from variables
MATRIX_TOKEN := $(subst ",,$(subst ',,$(MATRIX_TOKEN)))
MATRIX_HOMESERVER := $(subst ",,$(subst ',,$(MATRIX_HOMESERVER)))
MATRIX_ROOM_ID := $(subst ",,$(subst ',,$(MATRIX_ROOM_ID)))

export RUST_BACKTRACE ?= 1
export

.DEFAULT_GOAL := help


# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Build & main targets
# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.PHONY: build
build: ##H Build the Rust project
	@echo "Building ZK-Matrix-Join"
	$(CARGO) build --release

.PHONY: install
install: ##H Install the ruma-zk binary globally via cargo
	@echo "Installing ruma-zk..."
	$(CARGO) install --path . --force

.PHONY: setup
setup: ##H Fetch real Matrix data and Ruma state resolution fixtures
	@echo "Setting up project data and fixtures"
	@if [ -n "$$MATRIX_TOKEN" ]; then \
		echo "Fetching real Matrix state data from $$MATRIX_HOMESERVER"; \
		$(PYTHON) scripts/fetch_matrix_state.py; \
	else \
		echo "Skipping real Matrix fetch (MATRIX_TOKEN not set)."; \
	fi
	@echo "Ruma State Res test fixtures are checked in to res/"

.PHONY: clean
clean: ##H Clean up cache and temporary files
	@echo "Cleaning up"
	find . -name .mypy_cache -exec rm -rf {} +;
	find . -name .ruff_cache -exec rm -rf {} +;
	find . -name .pytest_cache -exec rm -rf {} +;
	rm -rf .tmp/coverage
# 	$(CARGO) clean


# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Testing
# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.PHONY: test
test: ##H Run fast Native Resolution tests (<1s)
	$(CARGO) test -p ruma-zk -- --nocapture
	$(CARGO) test -p ruma-lean -- --nocapture

.PHONY: test-full
test-full: ##H Run the full Jolt Parity Simulation (Slow)
	@echo "Running Deep Jolt Parity Simulation"
	RUST_LOG=info RAYON_NUM_THREADS=1 $(CARGO) test --release -p ruma-zk -- --ignored --nocapture

.PHONY: lint
lint: ##H Run clippy to lint the codebase
	$(CARGO) check
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	@if [ -n "$(VERBOSE)" ]; then \
		echo "Running ZK Security Scanner (vuln-002-VeilCash)"; \
		python3 scripts/detect_vuln_002.py; \
	fi

LINT_LOCS_PY ?= $(shell git ls-files '*.py')
.PHONY: format
format: ##H Format the Rust and Python codebase
	-pre-commit run --all-files
	-isort $(LINT_LOCS_PY)
	-black $(LINT_LOCS_PY)
	-prettier -w .

.PHONY: coverage
coverage: ##H Run workspace code coverage and generate HTML report
	@echo "Running focused workspace code coverage"
	$(CARGO) tarpaulin --out Html \
		--output-dir .tmp/coverage \
		--exclude-files "**/target/*" \
		--ignore-panics \
		--ignore-tests \
		--skip-clean



# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Demos & proving
# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

DEMO_INPUT ?= res/benchmark_1k.json
ifeq ($(TYPE),lite)
	DEMO_INPUT = res/ruma_bootstrap_events.json
endif

.PHONY: demo
demo: ##H Run the CLI Simulation (TYPE=lite for 5-event graph)
	@echo "Running ZK-Matrix-Join Demo (Input: $(DEMO_INPUT))..."
	$(CARGO) run --release -- demo --input $(DEMO_INPUT)

.PHONY: wasm
wasm: ##H Build the WebAssembly light-client Verifier
	@echo "Compiling WASM bindings"
	cd ruma-zk-wasm && wasm-pack build --target web

.PHONY: web-demo
web-demo: wasm ##H Run local web server to test WASM UI (Depends on wasm)
	@echo "================================================================"
	@echo " ZK-Matrix WebAssembly Server is starting!"
	@echo " http://localhost:8080/demo/index.html"
	@echo "================================================================"
	python3 -m http.server 8080

.PHONY: prove
prove: ##H Generate full Jolt STARK Proof
	@echo "Generating Jolt STARK Proof"
	RUST_LOG=info $(CARGO) run --release -- prove --input res/benchmark_1k.json

.PHONY: verify
verify: ##H Verify an existing Jolt STARK Proof
	@echo "Verifying Jolt STARK Proof"
	$(CARGO) run --release -- verify


# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# System & publishing
# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.PHONY: publish
publish: ##H Preview package file list and dry-run publish
	@echo "Previewing packaged files..."
	$(CARGO) package --list --allow-dirty
	@echo "Simulating publish..."
	$(CARGO) publish --dry-run --allow-dirty

.PHONY: cpu-info
cpu-info: ##H Print hardware info relevant to native targets
	@echo "=== CPU Model ==="
	@grep -m1 'model name' /proc/cpuinfo 2>/dev/null || sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown"
	@echo "=== rustc Native CPU ==="
	@rustc --print=cfg -C target-cpu=native 2>/dev/null | grep target_feature | sort
	@echo "=== Jolt VM Toolchain ==="
	@cargo jolt --version 2>/dev/null || echo "Jolt CLI not found"


# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Help
# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.PHONY: help
help: ##H Show this help, list available targets
	@grep -hE '^[a-zA-Z0-9_\/-]+:.*?##H .*$$' $(MAKEFILE_LIST) \
		        | awk 'BEGIN {FS = ":.*?##H "}; {printf "$(STYLE_CYAN)%-20s$(STYLE_RESET) %s\n", $$1, $$2}'
