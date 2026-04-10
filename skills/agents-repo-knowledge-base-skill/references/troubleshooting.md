# Troubleshooting

## `ctags is required`

`kb index regen` requires a `ctags` executable.

- macOS (Homebrew): install Universal Ctags.
- Linux: install `universal-ctags` (package name varies).

Validate:

```bash
ctags --version
```

## Rust toolchain not installed

This skill installs `kb` from GitHub release binaries by default and does not require Rust.

If network access is restricted, re-run onboarding with `--kb-bin </path/to/kb>`.

## Download fails (GitHub release)

The default `kb/tooling/install_kb.sh` uses GitHub release assets like:

- `kb-macos-arm64.tar.gz`
- `kb-macos-x86_64.tar.gz`
- `kb-linux-x86_64.tar.gz`

If the latest release does not have binaries for your platform, pin a known tag:

```bash
KB_TOOL_TAG=vX.Y.Z bash kb/tooling/install_kb.sh
```

## Hook does not run

Check where Git is sourcing hooks:

```bash
git config core.hooksPath
```

The onboarding script installs to the configured hooks path; if your environment overrides hooks execution, integrate `kb/tooling/kb-pre-commit.sh` into your existing hook runner.

## Index regen fails on a specific file

The `kb` indexer reads the Git-tracked path set. Common failure causes:

- non-UTF8 bytes in tracked files
- very large files causing backend issues

Mitigations:

- ensure the file is expected to be tracked
- consider excluding it from symbol extraction by policy (future); for now, keep the repo path set clean
