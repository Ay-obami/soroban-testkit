use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

/// A wrapper around [`soroban_sdk::Env`] that carries testkit state
/// (clock position, captured events, registered tokens) alongside the raw
/// SDK environment.
///
/// Every other module in this crate extends `TestEnv` with additional
/// methods (ledger control, event capture, token doubles, and so on) rather
/// than introducing separate handle types, so a single `TestEnv` is enough
/// to drive an entire test.
///
/// # Example
///
/// ```
/// use soroban_testkit::core::TestEnv;
///
/// let env = TestEnv::new();
/// let alice = env.address();
/// let bob = env.address();
/// assert_ne!(alice, bob);
/// ```
pub struct TestEnv {
    env: Env,
    // Consumed by the `money` module's seeded generators (Module 3).
    #[allow(dead_code)]
    seed: u64,
    // Per-environment override of the ledger close interval, in seconds.
    // `None` means "use the crate default"; the `ledger` module owns both
    // the default and the validation of overrides.
    close_interval_secs: Option<u64>,
}

impl TestEnv {
    /// Create a fresh environment with a deterministic starting ledger.
    ///
    /// Uses a non-reproducible seed for any future randomized value
    /// generation; use [`TestEnv::with_seed`] when a test needs to be
    /// reproducible.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let a = TestEnv::new();
    /// let b = TestEnv::new();
    /// // Independent environments: mutating one's ledger doesn't affect the other.
    /// use soroban_sdk::testutils::Ledger;
    /// a.env().ledger().set_sequence_number(1_000);
    /// assert_ne!(a.env().ledger().get().sequence_number, b.env().ledger().get().sequence_number);
    /// ```
    pub fn new() -> Self {
        Self::with_seed(random_seed())
    }

    /// Create an environment whose deterministic RNG seed is fixed, so that
    /// property tests using this crate's generators are reproducible.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let a = TestEnv::with_seed(42);
    /// let b = TestEnv::with_seed(42);
    /// assert_eq!(a.address(), b.address());
    /// ```
    pub fn with_seed(seed: u64) -> Self {
        Self {
            env: Self::fresh_env(),
            seed,
            close_interval_secs: None,
        }
    }

    /// Create a new, isolated environment that carries this one's
    /// configuration but none of its state.
    ///
    /// The result is built exactly as [`TestEnv::with_seed`] would build it,
    /// then given the settings this environment was configured with:
    ///
    /// | Carried over | Not carried over |
    /// |---|---|
    /// | the RNG seed ([`TestEnv::with_seed`]) | the ledger clock position (`now`, `sequence`) |
    /// | the ledger close interval, if one was set ([`TestEnv::with_ledger_close_interval`]) | deployed contracts and their storage |
    /// | | addresses, clients, and any other value created from this environment |
    /// | | ledger settings changed directly through [`TestEnv::env`] |
    ///
    /// An environment that never set a close interval stays that way: the
    /// clone follows the crate default rather than freezing today's value.
    ///
    /// The two environments share nothing afterwards — advancing the clock,
    /// deploying contracts, or generating addresses in one has no effect on
    /// the other. There is deliberately no `Clone` impl on `TestEnv` for the
    /// same reason: cloning a raw [`soroban_sdk::Env`] yields a second handle
    /// to the *same* underlying host, which would not be isolated.
    ///
    /// Only settings `TestEnv` itself tracks are carried over. Anything
    /// changed by reaching through [`TestEnv::env`] (for example
    /// `env().ledger().set(..)`) must be applied to the clone again.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    /// use std::time::Duration;
    ///
    /// let original = TestEnv::with_seed(7).with_ledger_close_interval(2);
    /// original.advance(Duration::from_secs(60));
    ///
    /// let copy = original.clone_config();
    /// // Configuration is carried over, the clock position is not...
    /// assert_eq!(copy.ledger_close_interval(), 2);
    /// assert_ne!(copy.now(), original.now());
    ///
    /// // ...and the two environments are isolated from each other.
    /// let before = original.now();
    /// copy.advance(Duration::from_secs(10));
    /// assert_eq!(original.now(), before);
    /// ```
    pub fn clone_config(&self) -> Self {
        Self {
            env: Self::fresh_env(),
            seed: self.seed,
            close_interval_secs: self.close_interval_secs,
        }
    }

    /// Discard all state in this environment and return it to the condition
    /// [`TestEnv::with_seed`] would have built it in, keeping its
    /// configuration.
    ///
    /// After `reset`, this environment behaves exactly like
    /// [`TestEnv::clone_config`] of itself would have: the seed and any
    /// ledger close interval are kept (see [`TestEnv::clone_config`] for the
    /// full list of what is and is not carried over), and everything else —
    /// the ledger clock position, deployed contracts and their storage — is
    /// gone. Addresses are issued from the start again, so the first
    /// [`TestEnv::address`] after a reset equals the first one a freshly
    /// built environment with the same seed would return.
    ///
    /// Values created before the reset (addresses, contract clients, cloned
    /// [`soroban_sdk::Env`] handles) stay bound to the discarded state and
    /// keep observing it; they are not moved into the reset environment.
    /// Recreate them after resetting rather than reusing them.
    ///
    /// `reset` takes `&mut self`, so the borrow checker rejects a reset while
    /// a `&Env` obtained from [`TestEnv::env`] is still alive.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    /// use std::time::Duration;
    ///
    /// let pristine = TestEnv::with_seed(7);
    /// let (start_now, start_sequence) = (pristine.now(), pristine.sequence());
    ///
    /// let mut env = TestEnv::with_seed(7).with_ledger_close_interval(2);
    /// env.advance(Duration::from_secs(600));
    /// assert_ne!(env.now(), start_now);
    ///
    /// env.reset();
    /// // The clock is back at the starting ledger; the configuration is kept.
    /// assert_eq!((env.now(), env.sequence()), (start_now, start_sequence));
    /// assert_eq!(env.ledger_close_interval(), 2);
    /// ```
    pub fn reset(&mut self) {
        self.env = Self::fresh_env();
    }

    /// Build the raw SDK environment every `TestEnv` starts from. Shared by
    /// construction, [`TestEnv::clone_config`] and [`TestEnv::reset`] so the
    /// three cannot drift apart.
    fn fresh_env() -> Env {
        Env::new_with_config(soroban_sdk::testutils::EnvTestConfig {
            capture_snapshot_at_drop: false,
        })
    }

    /// Escape hatch to the underlying SDK environment, for calls this crate
    /// does not wrap.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let _sdk_env: &soroban_sdk::Env = env.env();
    /// ```
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Generate a fresh random address.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let alice = env.address();
    /// let bob = env.address();
    /// assert_ne!(alice, bob);
    /// ```
    pub fn address(&self) -> Address {
        Address::generate(&self.env)
    }

    /// Generate `n` fresh addresses.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let addrs = env.addresses(3);
    /// assert_eq!(addrs.len(), 3);
    /// ```
    pub fn addresses(&self, n: usize) -> Vec<Address> {
        (0..n).map(|_| self.address()).collect()
    }

    /// The seed this environment was constructed with, for use by other
    /// modules' random value generators.
    #[allow(dead_code)]
    pub(crate) fn seed(&self) -> u64 {
        self.seed
    }

    /// The ledger close interval configured for this environment, if any.
    pub(crate) fn close_interval_override(&self) -> Option<u64> {
        self.close_interval_secs
    }

    /// Record a ledger close interval for this environment. Callers are
    /// responsible for validating `secs`.
    pub(crate) fn set_close_interval_override(&mut self, secs: u64) {
        self.close_interval_secs = Some(secs);
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

static SEED_COUNTER: AtomicU64 = AtomicU64::new(0);

fn random_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let count = SEED_COUNTER.fetch_add(1, Ordering::Relaxed);
    nanos ^ count.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Ledger;

    #[test]
    fn new_twice_produces_independent_environments() {
        let a = TestEnv::new();
        let b = TestEnv::new();
        a.env().ledger().set_sequence_number(12_345);
        assert_ne!(
            a.env().ledger().get().sequence_number,
            b.env().ledger().get().sequence_number
        );
    }

    #[test]
    fn with_seed_is_reproducible_across_runs() {
        let a = TestEnv::with_seed(42);
        let b = TestEnv::with_seed(42);
        assert_eq!(a.seed(), b.seed());
        assert_eq!(a.address(), b.address());
    }

    #[test]
    fn addresses_returns_n_distinct_addresses() {
        let env = TestEnv::new();
        let addrs = env.addresses(5);
        assert_eq!(addrs.len(), 5);
        for i in 0..addrs.len() {
            for j in (i + 1)..addrs.len() {
                assert_ne!(addrs[i], addrs[j]);
            }
        }
    }

    // --- clone_config ----------------------------------------------------

    #[test]
    fn clone_config_carries_the_seed() {
        let original = TestEnv::with_seed(42);
        assert_eq!(original.clone_config().seed(), 42);
    }

    #[test]
    fn clone_config_carries_the_close_interval() {
        let original = TestEnv::new().with_ledger_close_interval(2);
        assert_eq!(original.clone_config().ledger_close_interval(), 2);
    }

    #[test]
    fn clone_config_keeps_an_unset_close_interval_unset() {
        let clone = TestEnv::new().clone_config();
        assert_eq!(clone.close_interval_override(), None);
    }

    #[test]
    fn clone_config_does_not_carry_the_clock_position() {
        let original = TestEnv::new();
        original.advance_ledgers(25);
        let pristine = TestEnv::new();

        let clone = original.clone_config();
        assert_eq!(
            (clone.now(), clone.sequence()),
            (pristine.now(), pristine.sequence())
        );
        assert_ne!(clone.sequence(), original.sequence());
    }

    #[test]
    fn clone_config_matches_a_freshly_built_environment() {
        let original = TestEnv::with_seed(42);
        // Consume some addresses so any state leaking into the clone would show.
        original.addresses(3);
        assert_eq!(
            original.clone_config().address(),
            TestEnv::with_seed(42).address()
        );
    }

    #[test]
    fn clone_config_is_isolated_from_the_original() {
        let original = TestEnv::new();
        let clone = original.clone_config();
        let (original_before, clone_before) = (original.sequence(), clone.sequence());

        clone.env().ledger().set_sequence_number(9_000);
        assert_eq!(original.sequence(), original_before);

        original.env().ledger().set_sequence_number(7_000);
        assert_eq!(clone.sequence(), 9_000);
        assert_ne!(clone.sequence(), clone_before);
    }

    #[test]
    fn clone_config_of_a_clone_keeps_the_configuration() {
        let original = TestEnv::with_seed(5).with_ledger_close_interval(3);
        let second = original.clone_config().clone_config();
        assert_eq!(second.seed(), 5);
        assert_eq!(second.ledger_close_interval(), 3);
    }

    #[test]
    fn clone_config_does_not_change_the_original() {
        let original = TestEnv::with_seed(5).with_ledger_close_interval(3);
        original.advance_ledgers(4);
        let (now, sequence) = (original.now(), original.sequence());

        let _ = original.clone_config();
        assert_eq!((original.now(), original.sequence()), (now, sequence));
        assert_eq!(original.seed(), 5);
        assert_eq!(original.ledger_close_interval(), 3);
    }

    // --- reset -------------------------------------------------------------

    #[test]
    fn reset_returns_the_clock_to_the_starting_ledger() {
        let pristine = TestEnv::new();
        let mut env = TestEnv::new();
        env.advance_ledgers(50);
        assert_ne!(env.sequence(), pristine.sequence());

        env.reset();
        assert_eq!(
            (env.now(), env.sequence()),
            (pristine.now(), pristine.sequence())
        );
    }

    #[test]
    fn reset_keeps_the_seed_and_the_close_interval() {
        let mut env = TestEnv::with_seed(42).with_ledger_close_interval(2);
        env.advance_ledgers(10);

        env.reset();
        assert_eq!(env.seed(), 42);
        assert_eq!(env.ledger_close_interval(), 2);
    }

    #[test]
    fn reset_keeps_an_unset_close_interval_unset() {
        let mut env = TestEnv::new();
        env.reset();
        assert_eq!(env.close_interval_override(), None);
    }

    #[test]
    fn reset_issues_addresses_from_the_start_again() {
        let mut env = TestEnv::with_seed(42);
        env.addresses(4);

        env.reset();
        assert_eq!(env.address(), TestEnv::with_seed(42).address());
    }

    #[test]
    fn reset_leaves_earlier_handles_on_the_discarded_environment() {
        let mut env = TestEnv::new();
        env.advance_ledgers(3);
        let (now, sequence) = (env.now(), env.sequence());
        let stale = env.env().clone();

        env.reset();
        // The old handle still observes the old state...
        assert_eq!(stale.ledger().timestamp(), now);
        assert_eq!(stale.ledger().sequence(), sequence);
        // ...while the reset environment does not.
        assert_ne!(env.sequence(), sequence);
    }

    #[test]
    fn reset_environment_is_independent_of_the_discarded_one() {
        let mut env = TestEnv::new();
        let stale = env.env().clone();
        env.reset();

        let stale_before = stale.ledger().sequence();
        env.advance_ledgers(8);
        assert_eq!(stale.ledger().sequence(), stale_before);
    }

    #[test]
    fn reset_on_a_pristine_environment_changes_nothing_observable() {
        let mut env = TestEnv::with_seed(9);
        let (now, sequence) = (env.now(), env.sequence());

        env.reset();
        assert_eq!((env.now(), env.sequence()), (now, sequence));
        assert_eq!(env.seed(), 9);
    }

    #[test]
    fn reset_twice_is_the_same_as_reset_once() {
        let mut env = TestEnv::new().with_ledger_close_interval(4);
        env.advance_ledgers(6);

        env.reset();
        let once = (env.now(), env.sequence(), env.ledger_close_interval());
        env.reset();
        assert_eq!(
            (env.now(), env.sequence(), env.ledger_close_interval()),
            once
        );
    }

    #[test]
    fn reset_environment_is_fully_usable() {
        let mut env = TestEnv::new();
        env.advance_ledgers(2);
        env.reset();

        let before = env.sequence();
        env.advance_ledgers(5);
        assert_eq!(env.sequence(), before + 5);
        assert_ne!(env.address(), env.address());
    }
}
