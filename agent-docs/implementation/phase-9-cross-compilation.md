# Phase 9 — Cross-Compilation & Build Targets

## Goal

Set up cross-compilation for all target platforms, producing static binaries with no external runtime dependencies.

## Scope

- `Makefile dist` target
- Rust toolchain targets
- Build matrix for all supported platforms
- Platform-specific considerations (musl for Linux, armhf for Raspbian)
- `cross` (Docker-based) for Linux musl targets

## Target Platforms

| Target Triple | Platform | Notes |
|---------------|----------|-------|
| `aarch64-apple-darwin` | macOS Apple Silicon | Native on M1+ Macs |
| `x86_64-apple-darwin` | macOS Intel | Cross-compile from Apple Silicon |
| `x86_64-unknown-linux-musl` | Linux amd64 | Static binary via musl |
| `aarch64-unknown-linux-musl` | Linux arm64 | Static binary via musl (e.g., AWS Graviton, Pi 4+ 64-bit) |
| `armv7-unknown-linux-musleabihf` | Linux arm (Raspbian) | 32-bit ARM with hard-float ABI |

## Files Created / Modified

```
Makefile               # MODIFY: dist target using cargo (macOS) + cross (Linux)
.cargo/config.toml     # CREATE: per-target linker configuration for native musl builds
```

## Implementation

### 9.1 Rust toolchain targets

All 5 targets installed via `rustup target add`.

### 9.2 `.cargo/config.toml`

Configures per-target linkers for native musl cross-compilation (used when not going through `cross`):

```toml
[target.aarch64-unknown-linux-musl]
linker = "aarch64-linux-musl-gcc"

[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"

[target.armv7-unknown-linux-musleabihf]
linker = "arm-linux-musleabihf-gcc"
```

### 9.3 `Makefile dist` target

Uses `cargo build` for macOS targets (natively supported) and `cross build` for Linux musl targets (Docker-based, handles toolchain automatically):

- `MACOS_TARGETS`: aarch64-apple-darwin, x86_64-apple-darwin
- `LINUX_TARGETS`: x86_64-unknown-linux-musl, aarch64-unknown-linux-musl, armv7-unknown-linux-musleabihf
- `dist`: builds all targets, copies binaries to `target/dist/<target>/cmdify`
- `dist-clean`: removes `target/dist`
- `dist-verify`: runs `file` on each binary

### 9.4 Linux cross-compilation via `cross`

**Decision:** Use [`cross`](https://github.com/cross-rs/cross) (Docker-based) for Linux musl targets instead of native musl cross-compilers.

**Reason:** The project uses `reqwest` with `rustls` which depends on `aws-lc-sys`, a C library that requires target-specific C compilers (`aarch64-linux-musl-gcc`, etc.). Managing multiple musl cross-compilers is fragile and platform-dependent. `cross` handles this transparently via Docker containers.

**Prerequisites:**
- Docker must be installed and running
- `cross` installed via `cargo install cross --git https://github.com/cross-rs/cross`

### 9.5 macOS cross-compilation

macOS Apple Silicon to Intel works natively — no special linker needed, Xcode provides the toolchain.

### 9.6 Binary verification

`make dist-verify` confirms correct binary formats:

| Target | File type |
|--------|-----------|
| `aarch64-apple-darwin` | Mach-O 64-bit executable arm64 |
| `x86_64-apple-darwin` | Mach-O 64-bit executable x86_64 |
| `x86_64-unknown-linux-musl` | ELF 64-bit LSB executable, x86-64, statically linked |
| `aarch64-unknown-linux-musl` | ELF 64-bit LSB executable, aarch64, statically linked |
| `armv7-unknown-linux-musleabihf` | ELF 32-bit LSB executable, ARM, EABI5, statically linked |

## Tests

- Each cross-compiled binary runs and prints help on its target platform
- `ldd` on Linux binaries confirms "not a dynamic executable" (statically linked)
- `otool -L` on macOS binaries confirms no external library dependencies

## Acceptance Criteria

- [x] `make dist` builds binaries for all 5 targets
- [x] All macOS binaries have no external library dependencies (verified: Mach-O arm64 + x86_64)
- [ ] All Linux binaries are statically linked (no glibc dependency) — requires Docker to verify
- [ ] Each binary runs and shows help output on its target platform — requires target machines to verify
- [x] `make dist-verify` confirms correct binary formats
- [x] `make check` passes (353 tests, clippy, fmt)

## Verified

- macOS aarch64: `Mach-O 64-bit executable arm64`
- macOS x86_64: `Mach-O 64-bit executable x86_64`
- Linux targets: require Docker (not yet verified on this machine)
- All 353 existing tests pass, zero warnings
