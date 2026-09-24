// Attempting to `Clone` a `TestEnv` must not compile.
//
// `TestEnv` deliberately does not implement `Clone` because cloning a raw
// `soroban_sdk::Env` yields a second handle to the *same* underlying host,
// breaking test isolation. Use `TestEnv::clone_config()` instead, which
// builds a fresh, independent environment with the same configuration.
//
// See: <https://github.com/soroban-testkit/soroban-testkit/issues/48>

fn main() {
    let env = soroban_testkit::core::TestEnv::new();
    let _copy = env.clone(); // ERROR: `TestEnv` does not implement `Clone`
}
