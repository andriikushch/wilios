.PHONY: test test-all grammar-check install-deps install-extension install-skills example smoke build build-wilios build-mcp build-all grammar fmt

TARGETS = \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu \
	x86_64-pc-windows-gnu

fmt:
	cargo fmt --all

# Bare `cargo build --release` only covers default-members (crates/wilios-cli),
# so each binary gets its own target; `build` is just the aggregate.
build: build-wilios build-mcp

build-wilios:
	cargo build --release -p wilios-cli

build-mcp:
	cargo build --release -p wilios-mcp

build-all:
	@mkdir -p dist
	@for target in $(TARGETS); do \
		echo "Building $$target..."; \
		cross build --release -p wilios-cli -p wilios-mcp --target $$target || exit 1; \
		case $$target in \
			*windows*) ext=".exe" ;; \
			*) ext="" ;; \
		esac; \
		cp target/$$target/release/wilios$$ext dist/wilios-$$target$$ext; \
		cp target/$$target/release/wilios-mcp$$ext dist/wilios-mcp-$$target$$ext; \
	done

install-deps:
	rustup install nightly
	cargo install cargo-fuzz
	cargo install cross --git https://github.com/cross-rs/cross

test:
	# $(MAKECMDGOALS) is the full list of targets typed on the command line,
	# e.g. "test lex_simple_note_sharp".
	# $@ is the name of the current target ("test").
	# $(filter-out $@,$(MAKECMDGOALS)) strips the target name from that list,
	# leaving any extra words ("lex_simple_note_sharp") to forward to cargo as
	# a test-name filter.
	# Result: `make test lex_simple_note_sharp` → `cargo test lex_simple_note_sharp`
	cargo test --workspace $(filter-out $@,$(MAKECMDGOALS))

grammar-check:
	cargo test -p wilios-core --test grammar_check

test-all: test grammar-check
	cargo test -p wilios-core --test fuzz_props && \
	cargo +nightly fuzz run fuzz_lexer -- -max_total_time=30 && \
	cargo +nightly fuzz run fuzz_parser -- -max_total_time=30 && \
	cargo +nightly fuzz run fuzz_interpreter -- -max_total_time=30

example:
	RUST_LOG=debug cargo run -- examples/bebop_trio.wilios

# Headless CI check: the piece schedules with no runtime error and every track
# ends at the same nominal position. Runs against the through-composed example,
# the jazz-composer skill's teaching file, and the four idiom-library demos.
smoke:
	cargo run -q -- smoke examples/bebop_trio.wilios
	cargo run -q -- smoke examples/blues_f.wilios
	cargo run -q -- smoke examples/bebop_from_lib.wilios
	cargo run -q -- smoke examples/blues_from_lib.wilios
	cargo run -q -- smoke examples/bossa_from_lib.wilios
	cargo run -q -- smoke examples/modal_from_lib.wilios
	cargo run -q -- smoke examples/feel.wilios

grammar:
	python3 tools/gen_grammar.py

install-extension:
	@read -p "Install wilios VS Code extension to ~/.vscode/extensions? [y/N] " confirm && \
	[ "$$confirm" = "y" ] || [ "$$confirm" = "Y" ] || (echo "Aborted."; exit 1)
	@mkdir -p ~/.vscode/extensions
	@ln -sfn "$(CURDIR)/vscode-wilios" ~/.vscode/extensions/vscode-wilios
	@echo "Installed. Reload VS Code window to activate (Ctrl+Shift+P → Developer: Reload Window)."

# Symlink every skill under skills/ into .claude/skills/ so Claude Code discovers
# them. Symlinks (not copies) keep the installed skill live as you edit skills/.
# Re-run after adding a new skill; skills are picked up at Claude Code session start.
install-skills:
	@mkdir -p "$(CURDIR)/.claude/skills"
	@for d in "$(CURDIR)"/skills/*/; do \
		[ -f "$$d/SKILL.md" ] || continue; \
		name=$$(basename "$$d"); \
		ln -sfn "$${d%/}" "$(CURDIR)/.claude/skills/$$name"; \
		echo "linked .claude/skills/$$name -> skills/$$name"; \
	done