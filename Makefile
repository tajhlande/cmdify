.PHONY: build dev test lint fmt check clean install dist dist-clean dist-verify

MACOS_TARGETS = \
	aarch64-apple-darwin \
	x86_64-apple-darwin

LINUX_TARGETS = \
	x86_64-unknown-linux-musl \
	aarch64-unknown-linux-musl \
	armv7-unknown-linux-musleabihf

TARGETS = $(MACOS_TARGETS) $(LINUX_TARGETS)

DIST_DIR = target/dist

build:
	cargo build --release

dev:
	cargo build

test:
	cargo test

lint:
	cargo clippy -- -D warnings && cargo fmt --check

fmt:
	cargo fmt

check: lint test

clean:
	cargo clean

install:
	cargo install --path .

dist:
	@for target in $(MACOS_TARGETS); do \
		echo "Building for $$target (cargo)..."; \
		cargo build --release --target $$target || exit 1; \
		mkdir -p $(DIST_DIR)/$$target; \
		cp target/$$target/release/cmdify $(DIST_DIR)/$$target/; \
	done
	@for target in $(LINUX_TARGETS); do \
		echo "Building for $$target (cross)..."; \
		cross build --release --target $$target || exit 1; \
		mkdir -p $(DIST_DIR)/$$target; \
		cp target/$$target/release/cmdify $(DIST_DIR)/$$target/; \
	done
	@echo "All binaries built in $(DIST_DIR)/"

dist-clean:
	rm -rf $(DIST_DIR)

dist-verify:
	@for target in $(TARGETS); do \
		echo "=== $$target ==="; \
		file $(DIST_DIR)/$$target/cmdify; \
	done
