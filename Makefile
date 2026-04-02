.PHONY: build dev test lint fmt check clean install dist

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
	@for target in x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do \
		echo "Building for $$target..."; \
		cargo build --release --target $$target || true; \
	done
