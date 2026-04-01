# Phase 8 — CI/CD & Distribution

## Goal

Set up automated testing, linting, and release distribution via GitHub Actions. Polish the project for public use.

## Scope

- GitHub Actions CI workflow (test, lint, format check on every push/PR)
- GitHub Actions release workflow (build all targets, create GitHub release with binaries)
- Man page or usage documentation generation
- README with usage examples
- AGENTS.md cleanup (fill in stub sections per CRITIQUES.md #12/#20)

## Files to Create

```
.github/
└── workflows/
    ├── ci.yml              # CREATE: test + lint on push/PR
    └── release.yml         # CREATE: build + publish release on tag
README.md                   # CREATE: project overview, usage, installation
```

## Files to Modify

```
AGENTS.md                   # MODIFY: fill in Build process and Deployments stubs
```

## Implementation Steps

### 8.1 CI workflow (`.github/workflows/ci.yml`)

Per `BUILD.md §5.3`:

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test

  cross-check:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
```

### 8.2 Release workflow (`.github/workflows/release.yml`)

Triggered on git tags matching `v*`:

```yaml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-musl
          - aarch64-unknown-linux-musl
          - armv7-unknown-linux-musleabihf
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: rustup target add ${{ matrix.target }}
      - run: sudo apt install -y musl-tools gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: aicmd-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/aicmd

  build-macos:
    runs-on: macos-latest
    strategy:
      matrix:
        target:
          - aarch64-apple-darwin
          - x86_64-apple-darwin
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: rustup target add ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: aicmd-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/aicmd

  release:
    needs: [build, build-macos]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
      - uses: softprops/action-gh-release@v2
        with:
          files: aicmd-*/aicmd
```

### 8.3 README.md

- Project description
- Installation (cargo install, or download binary from releases)
- Quick start with env var configuration
- Usage examples for common providers
- CLI flags reference (`-q`, `-b`, `-n`)
- Environment variables reference (link to `BUILD.md`)
- How tools work (brief explanation)

### 8.4 AGENTS.md cleanup

Fill in the two empty stub sections:

- **Build process**: point to `Makefile` targets and `make check`
- **Deployments**: point to `make dist` and GitHub Actions release workflow

## Acceptance Criteria

- [ ] CI runs on every push to main and every PR (test, clippy, fmt)
- [ ] Release workflow builds all 5 targets and publishes to GitHub Releases on tag push
- [ ] README provides clear getting-started instructions
- [ ] AGENTS.md has no empty stub sections
- [ ] `make check` passes
