// Calling `reset()` on an immutably-borrowed `TestEnv` must not compile.
//
// `reset()` takes `&mut self` so the borrow checker rejects an attempt to
// reset while any other borrow of the same `TestEnv` is live, preventing
// state corruption across concurrent borrows.
//
// See: <https://github.com/soroban-testkit/soroban-testkit/issues/48>

fn main() {
    let env = soroban_testkit::core::TestEnv::new();
    let _ref: &soroban_testkit::core::TestEnv = &env;
    // This must fail: `reset` requires `&mut self` but `env` is immutably
    // borrowed by `_ref`.
    // Uncommenting would let us demonstrate it at compile time, but trybuild
    // tests must be written so the line that causes the error is present:
    env.reset(); // ERROR: cannot borrow `env` as mutable because it is also borrowed as immutable
    let _ = _ref; // extend the borrow past the reset call
}
