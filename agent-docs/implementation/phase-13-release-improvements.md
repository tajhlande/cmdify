# Phase 13 — Release Improvements

## Goal

Improve the quality and usability of release artifacts: Apple builds ship as DMGs, each artifact has its own checksum file, and artifact filenames include the release version for easy identification.

## Scope

- Apple distribution format changed from tar.gz to DMG (already done in release.yml)
- Individual `.sha256` checksum file per artifact (instead of one combined file)
- Artifact filenames include version number extracted from the git tag

## Files to Modify

```
.github/workflows/release.yml    # MODIFY: artifact naming, per-file checksums
RELEASE.md                        # MODIFY: update artifact names and verification instructions
agent-docs/implementation/PLAN.md # MODIFY: add Phase 13 row
```

## Implementation Steps

### 13.1 Apple DMG format

Already done. macOS builds use `hdiutil` to create DMGs instead of tar.gz.

### 13.2 Versioned artifact filenames

Extract the version from the git tag (`v0.12.0` → `0.12.0`) and include it in every artifact name.

**Current names:**
```
cmdify-aarch64-apple-darwin.dmg
cmdify-x86_64-unknown-linux-musl.tar.gz
```

**New names:**
```
cmdify-0.12.0-aarch64-apple-darwin.dmg
cmdify-0.12.0-x86_64-unknown-linux-musl.tar.gz
```

Implementation: use `${GITHUB_REF_NAME#v}` in the release workflow to strip the `v` prefix from the tag and pass it as an env var to each build job.

### 13.3 Individual checksum files

Instead of one `checksums-sha256.txt`, produce a `.sha256` file alongside each artifact:

```
cmdify-0.12.0-aarch64-apple-darwin.dmg
cmdify-0.12.0-aarch64-apple-darwin.dmg.sha256
cmdify-0.12.0-x86_64-apple-darwin.dmg
cmdify-0.12.0-x86_64-apple-darwin.dmg.sha256
...
```

Each `.sha256` file contains a single line: `<hash>  <filename>`

Users verify with:
```sh
sha256sum -c cmdify-0.12.0-aarch64-apple-darwin.dmg.sha256
```

### 13.4 Update release docs

Update `RELEASE.md` to reflect the new artifact names, verification instructions, and remove references to the combined checksum file.

## Acceptance Criteria

- [ ] macOS artifacts are DMGs with versioned filenames
- [ ] Linux artifacts are tar.gz with versioned filenames
- [ ] Each artifact has a corresponding `.sha256` checksum file
- [ ] No combined `checksums-sha256.txt` file
- [ ] `RELEASE.md` reflects the new artifact format
- [ ] `make check` passes
