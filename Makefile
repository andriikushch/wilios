.PHONY: test test-all grammar-check install-deps install-extension example build build-all grammar

TARGETS = \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu \
	x86_64-pc-windows-gnu

build:
	cargo build --release

build-all:
	@mkdir -p dist
	@for target in $(TARGETS); do \
		echo "Building $$target..."; \
		cross build --release --target $$target || exit 1; \
		case $$target in \
			*windows*) ext=".exe" ;; \
			*) ext="" ;; \
		esac; \
		cp target/$$target/release/wilios$$ext dist/wilios-$$target$$ext; \
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
	cargo test $(filter-out $@,$(MAKECMDGOALS))

grammar-check:
	cargo test --test grammar_check

test-all: test grammar-check
	cargo test --test fuzz_props && \
	cargo +nightly fuzz run fuzz_lexer -- -max_total_time=30 && \
	cargo +nightly fuzz run fuzz_parser -- -max_total_time=30 && \
	cargo +nightly fuzz run fuzz_interpreter -- -max_total_time=30

example:
	cargo run -- examples/example_swing.wilios

grammar:
	python3 tools/gen_grammar.py

install-extension:
	@read -p "Install wilios VS Code extension to ~/.vscode/extensions? [y/N] " confirm && \
	[ "$$confirm" = "y" ] || [ "$$confirm" = "Y" ] || (echo "Aborted."; exit 1)
	@mkdir -p ~/.vscode/extensions
	@ln -sfn "$(CURDIR)/vscode-wilios" ~/.vscode/extensions/vscode-wilios
	@echo "Installed. Reload VS Code window to activate (Ctrl+Shift+P → Developer: Reload Window)."