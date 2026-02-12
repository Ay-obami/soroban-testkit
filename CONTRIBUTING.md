# Contributing

## Scope

Read `BUILD_SPEC.md` §3 (Scope boundaries) before opening an issue or PR.
In scope: anything that helps test a Soroban contract in-process with
`soroban-sdk`'s test environment. Out of scope: mainnet/testnet forking,
deployment tooling, frontend/JS testing, a test runner, a full benchmarking
framework, anything requiring network access at test time.

## Workflow

- Trunk-based development: short-lived branches off `main`, squash merge,
  linear history.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/),
  enforced in CI.
- Every PR must pass: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test --workspace`, `cargo doc` with no warnings, `cargo audit`.
  Red CI blocks merge with no maintainer exception.

## Code conventions

- No `unwrap()` or `expect()` in library code outside tests.
- Assertion helpers panic deliberately (that is their contract), but always
  with `TestkitError` context, never a bare `panic!("...")` message.
- Every public item needs a doc comment with a runnable `# Example` block.
  `#![deny(missing_docs)]` is enforced from Module 8 onward.
- Every assertion helper needs two tests: one where it passes, one where it
  correctly fails (`#[should_panic(expected = "...")]`, pinning the
  message).
- Failure messages are a first-class feature of this crate. An assertion
  helper whose failure message doesn't tell the user what went wrong and
  what to look at is not done.

## Local setup

```sh
rustup show               # installs the pinned toolchain from rust-toolchain.toml
make check                # fmt + clippy + test, same as CI
```

## Good first issues

Labeled `good-first-issue`. Module 7 (`ttl`) is not beginner-friendly and
is never labeled as such.
