/// Compile-fail tests for common `soroban-testkit` API misuse patterns.
///
/// Each `.rs` file under `tests/compile_fail/` demonstrates a usage that
/// must be rejected by the Rust compiler. The accompanying `.stderr` file
/// pins the expected error message so that the test fails if the misuse
/// accidentally starts compiling, or if the error message changes in a
/// way that would confuse users.
///
/// These tests run via `trybuild`. To update the `.stderr` snapshots after
/// an intentional change, delete the relevant `.stderr` file and run
/// `cargo test compile_fail` — `trybuild` will write fresh snapshots.
#[test]
fn compile_fail_examples() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
