# Releasing cmdify

## Overview

Releases are triggered by pushing a git tag matching `v*` (e.g., `v0.11.0`). GitHub Actions builds binaries for all 5 supported targets, packages them as tar.gz, generates SHA-256 checksums, and publishes everything to GitHub Releases.

## Prerequisites

- Push access to the repository
- All work for the release merged to `main`
- `make check` passing

## Steps

### 1. Update the version in Cargo.toml

Edit `Cargo.toml` and bump the `version` field. Follow semver:

- **Patch** (bug fixes): `0.10.0` → `0.10.1`
- **Minor** (new features): `0.10.0` → `0.11.0`
- **Major** (breaking changes): `0.10.0` → `1.0.0`

### 2. Verify everything is green and lock file is current

```sh
make check
cargo build
git diff --exit-code Cargo.lock
```

If the last command shows a diff, `Cargo.lock` was out of sync with `Cargo.toml` — stage and include it in your commit.

### 3. Update phase docs and README

- Update `README.md` roadmap table if a new phase was completed
- Update `agent-docs/implementation/PLAN.md` phase status if applicable
- Update the relevant phase doc with implementation notes if applicable

### 4. Commit all changes

```sh
git add -A
git commit -m "prepare release v0.11.0"
```

### 5. Tag and push

The tag **must** start with `v` and match the version in `Cargo.toml`:

```sh
git tag v0.11.0
git push && git push --tags
```

### 6. Monitor the build

Go to the repository's **Actions** tab on GitHub. The "Release" workflow will run. It builds:

| Target | Platform | Method |
|--------|----------|--------|
| `aarch64-apple-darwin` | macOS Apple Silicon | native `cargo` |
| `x86_64-apple-darwin` | macOS Intel | native `cargo` |
| `x86_64-unknown-linux-musl` | Linux amd64 | `cross` (Docker) |
| `aarch64-unknown-linux-musl` | Linux arm64 | `cross` (Docker) |
| `armv7-unknown-linux-musleabihf` | Linux arm (Raspbian) | `cross` (Docker) |

Linux builds take longer due to Docker setup.

### 7. Verify the release

Once the workflow completes, go to the **Releases** page. Confirm:

- All 5 `cmdify-<target>.tar.gz` files are attached
- `checksums-sha256.txt` is attached
- Release notes were auto-generated from commit messages

Users verify downloads with:

```sh
sha256sum -c checksums-sha256.txt
```

## What if the build fails?

Delete the tag locally and remotely, fix the issue, and re-tag:

```sh
git tag -d v0.11.0
git push origin :refs/tags/v0.11.0
# fix the issue, commit, push
git tag v0.11.0
git push --tags
```

## Reproducibility

Every release is fully reproducible from its tag:

- The tag pins the exact source commit
- `Cargo.lock` pins exact dependency versions
- Anyone can rebuild from a tag: `git checkout v0.11.0 && cargo build --release`

## Artifacts

Each release produces:

- `cmdify-aarch64-apple-darwin.tar.gz`
- `cmdify-x86_64-apple-darwin.tar.gz`
- `cmdify-x86_64-unknown-linux-musl.tar.gz`
- `cmdify-aarch64-unknown-linux-musl.tar.gz`
- `cmdify-armv7-unknown-linux-musleabihf.tar.gz`
- `checksums-sha256.txt`
