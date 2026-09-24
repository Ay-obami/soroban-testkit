//! Regression test: README examples cannot silently drift from the
//! public API.
//!
//! `README.md` is compiled into the crate's rustdoc via
//! `#![doc = include_str!]` in `lib.rs`, which makes its Rust blocks
//! doctests — but only while (a) that include stays in place, (b)
//! `doctest = true` stays enabled, and (c) CI actually runs the doc
//! tests. Doc tests only run as a side effect of `cargo test`, and
//! `cargo test --all-targets` (the command this project's own issue
//! template suggests) skips them, so (c) used to be accidental. Each
//! link in that chain is pinned here.

mod common;

use common::read_repo_file;

#[test]
fn readme_examples_cannot_silently_drift_from_the_public_api() {
    let lib_rs = read_repo_file("crates/testkit/src/lib.rs");
    assert!(
        lib_rs.contains("#![doc = include_str!(\"../../../README.md\")]"),
        "crates/testkit/src/lib.rs must compile README.md into the crate docs with \
         `#![doc = include_str!(\"../../../README.md\")]`. Without it the README \
         examples are no longer doctests and can drift from the public API unnoticed."
    );

    let manifest = read_repo_file("crates/testkit/Cargo.toml");
    assert!(
        manifest.contains("doctest = true"),
        "crates/testkit/Cargo.toml must keep `doctest = true`; disabling it \
         silently drops every README example from the test suite."
    );

    let readme = read_repo_file("README.md");
    assert!(
        readme.contains("use soroban_testkit::"),
        "README.md must contain at least one Rust example using `soroban_testkit`, \
         so the doctest wiring above has something real to compile against the \
         public API."
    );

    let ci = read_repo_file(".github/workflows/ci.yml");
    assert!(
        ci.contains("test-no-network.sh doc"),
        ".github/workflows/ci.yml must run the doc tests as their own step via \
         `test-no-network.sh doc`. As a side effect of a single generic test step \
         they vanish the moment that step's flags change — `--all-targets` skips \
         doctests entirely — and README examples start drifting in silence."
    );

    let script = read_repo_file(".github/scripts/test-no-network.sh");
    assert!(
        script.contains("cargo test --workspace --doc"),
        "test-no-network.sh's `doc` mode must map to `cargo test --workspace --doc`; \
         without the `--doc` flag no README example is compiled or executed."
    );

    let makefile = read_repo_file("Makefile");
    assert!(
        makefile.contains("cargo test --workspace --doc"),
        "the Makefile `test` target must run `cargo test --workspace --doc` so \
         `make check` exercises the same README-doctest gate as CI."
    );
}
