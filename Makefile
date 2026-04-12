SHELL=/bin/bash
.DEFAULT_GOAL=_help

LAKE ?= ~/.elan/bin/lake


# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Init and format
# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.PHONY: cache
cache: ##H Update Lean cache
	$(LAKE) exe cache get


LINT_LOCS_LEAN = $$(git ls-files '**/*.lean')
LINT_LOCS_PY = $$(git ls-files '*.py')
LINT_LOCS_SH = $$(git ls-files '*.sh')

.PHONY: format
format: ##H Format codebase
	-prettier -w .
	-pre-commit run --all-files
	-black $(LINT_LOCS_PY)
	-isort $(LINT_LOCS_PY)
	-shfmt -w $(LINT_LOCS_SH)

.PHONY: clean
clean: ##H Remove build artifacts
	-$(LAKE) clean
	-$(CARGO) clean
	# rm -rf res/ .tmp/ .lake/build/



# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Main target
# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.PHONY: lean
lean: ##H Run Lean theorem proofs and verification
	$(LAKE) build
	@printf "\n$${STYLE_GREEN}--- Verification Complete ---$${STYLE_RESET}\n"
	@printf "$${STYLE_CYAN}Mapped Theorems & Definitions:$${STYLE_RESET}\n"
	@grep -E '^(theorem|def|class|instance|structure) ' ctopology/lean_src/ctopology/*.lean ctopology/lean_src/ctopology.lean || true
	@printf "$${STYLE_GREEN}--------------------------------$${STYLE_RESET}\n"

.PHONY: docs
docs: ##H Generate Lean docs
	DOCGEN_SRC="file" DOCGEN_SKIP_LEAN=1 DOCGEN_SKIP_STD=1 DOCGEN_SKIP_LAKE=1 DOCGEN_SKIP_DEPS=1 $(LAKE) build ctopology:docs

.PHONY: bench
bench: ##H Run high-performance O(N) benchmark
	cd ctopology && cargo run --release



# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Help & support commands
# ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

# [ENUM] Styling / Colors
STYLE_CYAN := $(shell tput setaf 6 2>/dev/null || echo '\033[36m')
STYLE_GREEN := $(shell tput setaf 2 2>/dev/null || echo '\033[32m')
STYLE_RESET := $(shell tput sgr0 2>/dev/null || echo '\033[0m')
export STYLE_CYAN STYLE_GREEN STYLE_RESET

.PHONY: _help
_help:
	@grep -hE '^[a-zA-Z0-9_\/-]+:[[:space:]]*##H .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":[[:space:]]*##H "}; {printf "$(STYLE_CYAN)%-15s$(STYLE_RESET) %s\n", $$1, $$2}'
