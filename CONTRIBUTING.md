# Contributing to esp-idf-improv-wifi

## Getting Started

```bash
git clone https://github.com/nightwatch-astro/esp-idf-improv-wifi.git
cd esp-idf-improv-wifi
cargo build --no-default-features
cargo test --no-default-features
```

The `esp-idf-svc` feature requires the ESP-IDF toolchain and is not needed for host development.

## Development

### Building

```bash
cargo build --no-default-features          # host-only (no ESP-IDF dependency)
cargo check --no-default-features          # quick type check
```

### Testing

```bash
cargo test --no-default-features           # unit tests
cargo clippy --no-default-features -- -D warnings  # lint
cargo fmt --check                          # format check
```

### On-device testing

To build with ESP-IDF support, you need the [esp-idf-svc](https://github.com/esp-rs/esp-idf-svc) toolchain installed:

```bash
cargo build --features esp-idf-svc         # requires ESP-IDF toolchain
```

## Architecture

```
src/
  lib.rs             Public API, protocol types, state machine
```

The crate is structured as a `no_std`-friendly library with an optional `esp-idf-svc` feature that provides the concrete serial transport implementation. The core protocol logic (packet parsing, state machine, command handling) compiles on any host target.

## Pull Request Process

1. Fork and create a feature branch from `main`
2. Make your changes with conventional commit messages
3. Ensure CI passes: `cargo test`, `cargo clippy`, `cargo fmt --check` (all with `--no-default-features`)
4. Open a PR — the template will guide you through the checklist

## Commit Convention

This project uses [conventional commits](https://www.conventionalcommits.org/):

```
feat: add WiFi scan result forwarding
fix: handle malformed RPC response gracefully
docs: add serial wiring example
test: state machine transition coverage
refactor: extract packet parser into module
```

## Code Style

- `cargo fmt` for formatting
- `cargo clippy` for linting
- Functions return `Result` types — avoid panics
- Keep the core protocol logic independent of `esp-idf-svc`

## Contributor License Agreement (CLA)

This project requires a CLA for all contributions. When you open your first pull
request, the CLA Assistant bot will comment with a link to review and sign the
agreement. You only need to sign once — it covers all future contributions to
this repository.

**Why?** The CLA ensures the project maintainer retains the ability to offer the
code under alternative licensing terms (e.g., a commercial license) without
needing to track down every contributor individually. Your contribution remains
publicly available under the project's open-source license (MPL-2.0) regardless.

## License

By contributing, you agree that your contributions will be licensed under the Mozilla Public License, Version 2.0.
