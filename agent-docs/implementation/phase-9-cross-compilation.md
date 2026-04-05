# Phase 9 — Cross-Compilation & Build Targets

## Goal

Set up cross-compilation for all target platforms, producing static binaries with no external runtime dependencies.

## Scope

- `Makefile dist` target
- Rust toolchain targets
- Build matrix for all supported platforms
- Platform-specific considerations (musl for Linux, armhf for Raspbian)

## Target Platforms

| Target Triple | Platform | Notes |
|---------------|----------|-------|
| `aarch64-apple-darwin` | macOS Apple Silicon | Native on M1+ Macs |
| `x86_64-apple-darwin` | macOS Intel | Cross-compile from Apple Silicon |
| `x86_64-unknown-linux-musl` | Linux amd64 | Static binary via musl |
| `aarch64-unknown-linux-musl` | Linux arm64 | Static binary via musl (e.g., AWS Graviton, Pi 4+ 64-bit) |
| `armv7-unknown-linux-musleabihf` | Linux arm (Raspbian) | 32-bit ARM with hard-float ABI |

## Files to Create / Modify

```
Makefile        # MODIFY: add dist target, cross-compilation
.cargo/config.toml  # CREATE: per-target linker configuration
```

## Implementation Steps

### 9.1 Rust toolchain targets

Add required targets:

```sh
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-unknown-linux-musl
rustup target add armv7-unknown-linux-musleabihf
```

### 9.2 `.cargo/config.toml`

Configure per-target settings:

```toml
[target.aarch64-unknown-linux-musl]
linker = "aarch64-linux-musl-gcc"

[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"

[target.armv7-unknown-linux-musleabihf]
linker = "arm-linux-musleabihf-gcc"
```

Note: musl cross-compilers must be installed on the build host (e.g., via `brew install filosottile/musl-cross/musl-cross` on macOS, or `apt install musl-tools` on Linux).

### 9.3 `Makefile dist` target

```makefile
TARGETS = \
	aarch64-apple-darwin \
	x86_64-apple-darwin \
	x86_64-unknown-linux-musl \
	aarch64-unknown-linux-musl \
	armv7-unknown-linux-musleabihf

DIST_DIR = target/dist

dist:
	@for target in $(TARGETS); do \
		echo "Building for $$target..."; \
		cargo build --release --target $$target; \
		mkdir -p $(DIST_DIR)/$$target; \
		cp target/$$target/release/cmdify $(DIST_DIR)/$$target/; \
	done
	@echo "All binaries built in $(DIST_DIR)/"

dist-clean:
	rm -rf $(DIST_DIR)
```

### 9.4 Platform-specific considerations

**macOS Apple Silicon → Intel cross-compilation:**
- Works natively with `rustup target add x86_64-apple-darwin`
- No special linker needed (Xcode provides it)
- Requires macOS SDK for both architectures

**macOS → Linux cross-compilation:**
- Requires musl cross-compilers installed on the build host
- On macOS: `brew install filosottile/musl-cross/musl-cross` (for x86_64 and aarch64)
- On Linux: `apt install musl-tools` (for x86_64 only; aarch64 and arm need `cross` or Docker)

**Raspbian (armv7):**
- Uses `armv7-unknown-linux-musleabihf` target
- Requires `arm-linux-musleabihf-gcc` cross-compiler
- Binary works on Raspberry Pi 2+ running Raspberry Pi OS (32-bit)
- For 64-bit Pi OS, use `aarch64-unknown-linux-musl` instead

**Alternative: `cross` crate:**
- If native cross-compilation is too painful, use `cross` (Docker-based)
- `cross build --release --target <target>` handles toolchain setup automatically
- Add a `Cross.toml` if needed for custom Docker images

### 9.5 Binary verification

After building, verify each binary:

```makefile
dist-verify:
	@for target in $(TARGETS); do \
		file $(DIST_DIR)/$$target/cmdify; \
	done
```

Expected output:

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

- [ ] `make dist` builds binaries for all 5 targets
- [ ] All Linux binaries are statically linked (no glibc dependency)
- [ ] All macOS binaries have no external library dependencies
- [ ] Each binary runs and shows help output on its target platform
- [ ] `make dist-verify` confirms correct binary formats
