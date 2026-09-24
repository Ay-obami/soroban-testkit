// Sending a `TestEnv` across a thread boundary must not compile.
//
// `TestEnv` is not `Send` because `soroban_sdk::Env` is not `Send`.
// Each `TestEnv` is meant to be owned and driven from a single thread.
// Create one `TestEnv` per thread/task instead.
//
// See: <https://github.com/soroban-testkit/soroban-testkit/issues/48>

fn require_send<T: Send>(_: T) {}

fn main() {
    let env = soroban_testkit::core::TestEnv::new();
    require_send(env); // ERROR: `TestEnv` is not `Send`
}
