use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Ledger as _;
use soroban_sdk::{Address, Env};

// ──────────────────────────────────────────────────────────────────────────
// Issue #35 — builder for deterministic ledger defaults
// ──────────────────────────────────────────────────────────────────────────

/// A builder for the ledger parameters a [`TestEnv`] starts with.
///
/// `LedgerDefaults` lets you pin the initial `timestamp`, `sequence_number`,
/// `protocol_version`, and `base_reserve` so that every environment built
/// from it produces identical starting conditions. Use it when a test needs
/// a specific epoch or a particular protocol version, and you want to state
/// that intent up front rather than calling `env.warp_to(…)` or reaching
/// through `env.env().ledger().set(…)` after construction.
///
/// Unset fields keep the [`soroban_sdk::Env`] defaults (all zeros / SDK
/// defaults as of the pinned `soroban-sdk` version).
///
/// # Example
///
/// ```
/// use soroban_testkit::core::{LedgerDefaults, TestEnv};
///
/// let defaults = LedgerDefaults::new()
///     .timestamp(1_700_000_000)
///     .sequence_number(500_000);
///
/// let env = TestEnv::with_ledger_defaults(defaults);
/// assert_eq!(env.now(), 1_700_000_000);
/// assert_eq!(env.sequence(), 500_000);
/// ```
#[derive(Debug, Clone, Default)]
pub struct LedgerDefaults {
    pub(crate) timestamp: Option<u64>,
    pub(crate) sequence_number: Option<u32>,
    pub(crate) protocol_version: Option<u32>,
    pub(crate) base_reserve: Option<u32>,
}

impl LedgerDefaults {
    /// Create a builder with no overrides; all fields keep the SDK defaults.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::LedgerDefaults;
    ///
    /// let defaults = LedgerDefaults::new();
    /// // All fields are None; the SDK defaults apply.
    /// assert!(defaults.timestamp_override().is_none());
    /// assert!(defaults.sequence_number_override().is_none());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the initial unix timestamp (whole seconds).
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{LedgerDefaults, TestEnv};
    ///
    /// let env = TestEnv::with_ledger_defaults(
    ///     LedgerDefaults::new().timestamp(1_700_000_000),
    /// );
    /// assert_eq!(env.now(), 1_700_000_000);
    /// ```
    pub fn timestamp(mut self, value: u64) -> Self {
        self.timestamp = Some(value);
        self
    }

    /// Set the initial ledger sequence number.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{LedgerDefaults, TestEnv};
    ///
    /// let env = TestEnv::with_ledger_defaults(
    ///     LedgerDefaults::new().sequence_number(42_000),
    /// );
    /// assert_eq!(env.sequence(), 42_000);
    /// ```
    pub fn sequence_number(mut self, value: u32) -> Self {
        self.sequence_number = Some(value);
        self
    }

    /// Set the initial protocol version.
    ///
    /// The protocol version must be compatible with the version of
    /// `soroban-sdk` in use (too old a value is rejected by the host).
    /// Prefer using the current version from the SDK as a baseline when you
    /// need to override this field.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{LedgerDefaults, TestEnv};
    /// use soroban_sdk::testutils::Ledger as _;
    ///
    /// // Resolve the current protocol version from the SDK itself so that
    /// // this example compiles against any soroban-sdk version.
    /// let current = {
    ///     let base = soroban_sdk::Env::default();
    ///     base.ledger().get().protocol_version
    /// };
    /// let env = TestEnv::with_ledger_defaults(
    ///     LedgerDefaults::new().protocol_version(current),
    /// );
    /// assert_eq!(env.env().ledger().get().protocol_version, current);
    /// ```
    pub fn protocol_version(mut self, value: u32) -> Self {
        self.protocol_version = Some(value);
        self
    }

    /// Set the initial base reserve (in stroops).
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{LedgerDefaults, TestEnv};
    ///
    /// let env = TestEnv::with_ledger_defaults(
    ///     LedgerDefaults::new().base_reserve(5_000_000),
    /// );
    /// drop(env);
    /// ```
    pub fn base_reserve(mut self, value: u32) -> Self {
        self.base_reserve = Some(value);
        self
    }

    /// The configured timestamp override, if any.
    pub fn timestamp_override(&self) -> Option<u64> {
        self.timestamp
    }

    /// The configured sequence number override, if any.
    pub fn sequence_number_override(&self) -> Option<u32> {
        self.sequence_number
    }

    /// The configured protocol version override, if any.
    pub fn protocol_version_override(&self) -> Option<u32> {
        self.protocol_version
    }

    /// The configured base reserve override, if any.
    pub fn base_reserve_override(&self) -> Option<u32> {
        self.base_reserve
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Issue #39 — test-environment metadata in diagnostic output
// ──────────────────────────────────────────────────────────────────────────

/// A snapshot of [`TestEnv`] state for inclusion in assertion failure
/// messages.
///
/// Every assertion helper in this crate that can fail includes an
/// `EnvMetadata` snapshot in its failure message so that the developer
/// immediately sees *where* in ledger time the failure happened, without
/// having to add manual `println!` calls or re-run under a debugger.
///
/// Use [`TestEnv::metadata`] to capture a snapshot at any point.
///
/// # Example
///
/// ```
/// use soroban_testkit::core::TestEnv;
/// use std::time::Duration;
///
/// let env = TestEnv::with_seed(1);
/// env.advance(Duration::from_secs(300));
///
/// let meta = env.metadata();
/// let display = meta.to_string();
/// assert!(display.contains("timestamp=300"));
/// assert!(display.contains("sequence="));
/// assert!(display.contains("seed=1"));
/// ```
#[derive(Debug, Clone)]
pub struct EnvMetadata {
    /// Current ledger timestamp (unix seconds).
    pub timestamp: u64,
    /// Current ledger sequence number.
    pub sequence: u32,
    /// Current ledger protocol version.
    pub protocol_version: u32,
    /// The RNG seed this environment was built with.
    pub seed: u64,
    /// The configured ledger close interval (seconds), if any was set.
    pub close_interval_override: Option<u64>,
}

impl fmt::Display for EnvMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestEnv {{ timestamp={}, sequence={}, protocol_version={}, seed={}",
            self.timestamp, self.sequence, self.protocol_version, self.seed,
        )?;
        if let Some(interval) = self.close_interval_override {
            write!(f, ", close_interval={interval}s")?;
        }
        write!(f, " }}")
    }
}

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
    // Deterministic starting ledger parameters provided via LedgerDefaults.
    ledger_defaults: LedgerDefaults,
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
            env: Self::fresh_env_with_defaults(&LedgerDefaults::default()),
            seed,
            close_interval_secs: None,
            ledger_defaults: LedgerDefaults::default(),
        }
    }

    /// Create an environment that starts from the given [`LedgerDefaults`].
    ///
    /// This is the primary entry point when a test depends on a specific
    /// epoch, protocol version, or sequence number. Combining it with
    /// [`TestEnv::with_seed`] via [`TestEnv::with_ledger_defaults_and_seed`]
    /// makes the full starting state deterministic.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{LedgerDefaults, TestEnv};
    ///
    /// let defaults = LedgerDefaults::new()
    ///     .timestamp(1_700_000_000)
    ///     .sequence_number(1_000_000);
    ///
    /// let env = TestEnv::with_ledger_defaults(defaults);
    /// assert_eq!(env.now(), 1_700_000_000);
    /// assert_eq!(env.sequence(), 1_000_000);
    /// ```
    pub fn with_ledger_defaults(defaults: LedgerDefaults) -> Self {
        Self {
            env: Self::fresh_env_with_defaults(&defaults),
            seed: random_seed(),
            close_interval_secs: None,
            ledger_defaults: defaults,
        }
    }

    /// Create an environment with a fixed RNG seed and deterministic starting
    /// ledger.
    ///
    /// Combines [`TestEnv::with_seed`] and [`TestEnv::with_ledger_defaults`]
    /// so that both the randomised value generators and the initial clock
    /// position are fully reproducible.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{LedgerDefaults, TestEnv};
    ///
    /// let defaults = LedgerDefaults::new().timestamp(1_000_000);
    /// let a = TestEnv::with_ledger_defaults_and_seed(defaults.clone(), 99);
    /// let b = TestEnv::with_ledger_defaults_and_seed(defaults, 99);
    ///
    /// assert_eq!(a.now(), b.now());
    /// assert_eq!(a.address(), b.address());
    /// ```
    pub fn with_ledger_defaults_and_seed(defaults: LedgerDefaults, seed: u64) -> Self {
        Self {
            env: Self::fresh_env_with_defaults(&defaults),
            seed,
            close_interval_secs: None,
            ledger_defaults: defaults,
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
            env: Self::fresh_env_with_defaults(&self.ledger_defaults),
            seed: self.seed,
            close_interval_secs: self.close_interval_secs,
            ledger_defaults: self.ledger_defaults.clone(),
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
        self.env = Self::fresh_env_with_defaults(&self.ledger_defaults);
    }

    /// Build the raw SDK environment every `TestEnv` starts from. Shared by
    /// construction, [`TestEnv::clone_config`] and [`TestEnv::reset`] so the
    /// three cannot drift apart.
    fn fresh_env_with_defaults(defaults: &LedgerDefaults) -> Env {
        let env = Env::new_with_config(soroban_sdk::testutils::EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        // Apply any LedgerDefaults overrides so the starting ledger is
        // deterministic. We only mutate the fields the caller specified;
        // unset fields keep the SDK's own defaults.
        if defaults.timestamp.is_some()
            || defaults.sequence_number.is_some()
            || defaults.protocol_version.is_some()
            || defaults.base_reserve.is_some()
        {
            let mut info = env.ledger().get();
            if let Some(ts) = defaults.timestamp {
                info.timestamp = ts;
            }
            if let Some(seq) = defaults.sequence_number {
                info.sequence_number = seq;
            }
            if let Some(pv) = defaults.protocol_version {
                info.protocol_version = pv;
            }
            if let Some(br) = defaults.base_reserve {
                info.base_reserve = br;
            }
            env.ledger().set(info);
        }
        env
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

    // ──────────────────────────────────────────────────────────────────────
    // Issue #35 — LedgerDefaults accessor
    // ──────────────────────────────────────────────────────────────────────

    /// The [`LedgerDefaults`] this environment was built with.
    ///
    /// Useful for cloning configuration or inspecting what was requested when
    /// an assertion produces a failure message.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{LedgerDefaults, TestEnv};
    ///
    /// let defaults = LedgerDefaults::new().timestamp(1_000_000);
    /// let env = TestEnv::with_ledger_defaults(defaults);
    /// assert_eq!(env.ledger_defaults().timestamp_override(), Some(1_000_000));
    /// ```
    pub fn ledger_defaults(&self) -> &LedgerDefaults {
        &self.ledger_defaults
    }

    // ──────────────────────────────────────────────────────────────────────
    // Issue #39 — test-environment metadata
    // ──────────────────────────────────────────────────────────────────────

    /// Capture a snapshot of this environment's current state for use in
    /// assertion failure messages.
    ///
    /// [`EnvMetadata`] implements [`std::fmt::Display`] so it can be
    /// interpolated directly into a `format!` string. All assertion helpers
    /// in this crate include a metadata snapshot in their failure output so
    /// that a failed test tells you *where* in ledger time and *with which
    /// seed* it failed.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    /// use std::time::Duration;
    ///
    /// let env = TestEnv::with_seed(7);
    /// env.advance(Duration::from_secs(120));
    ///
    /// let meta = env.metadata();
    /// assert_eq!(meta.seed, 7);
    /// assert_eq!(meta.timestamp, 120);
    ///
    /// let display = meta.to_string();
    /// assert!(display.contains("seed=7"), "{display}");
    /// assert!(display.contains("timestamp=120"), "{display}");
    /// ```
    pub fn metadata(&self) -> EnvMetadata {
        let info = self.env().ledger().get();
        EnvMetadata {
            timestamp: info.timestamp,
            sequence: info.sequence_number,
            protocol_version: info.protocol_version,
            seed: self.seed,
            close_interval_override: self.close_interval_secs,
        }
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

    // ─────────────────────────────────────────────────────────────────────
    // Issue #35 — LedgerDefaults builder
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn ledger_defaults_new_has_all_none() {
        let d = LedgerDefaults::new();
        assert!(d.timestamp_override().is_none());
        assert!(d.sequence_number_override().is_none());
        assert!(d.protocol_version_override().is_none());
        assert!(d.base_reserve_override().is_none());
    }

    #[test]
    fn ledger_defaults_timestamp_sets_the_starting_timestamp() {
        let env = TestEnv::with_ledger_defaults(LedgerDefaults::new().timestamp(1_700_000_000));
        assert_eq!(env.now(), 1_700_000_000);
    }

    #[test]
    fn ledger_defaults_sequence_sets_the_starting_sequence() {
        let env = TestEnv::with_ledger_defaults(LedgerDefaults::new().sequence_number(999_000));
        assert_eq!(env.sequence(), 999_000);
    }

    #[test]
    fn ledger_defaults_protocol_version_is_applied() {
        // Use the SDK's own current protocol version to avoid the "too old"
        // check in soroban-env-host (which rejects versions below the host's
        // interface version in non-test-mode builds of that crate).
        let current = soroban_sdk::Env::default().ledger().get().protocol_version;
        let env = TestEnv::with_ledger_defaults(LedgerDefaults::new().protocol_version(current));
        assert_eq!(env.env().ledger().get().protocol_version, current);
    }

    #[test]
    fn ledger_defaults_base_reserve_is_applied() {
        let env = TestEnv::with_ledger_defaults(LedgerDefaults::new().base_reserve(5_000_000));
        assert_eq!(env.env().ledger().get().base_reserve, 5_000_000);
    }

    #[test]
    fn ledger_defaults_all_fields_together() {
        // Use the SDK's current protocol version to avoid the "too old" check.
        let current_protocol = soroban_sdk::Env::default().ledger().get().protocol_version;
        let env = TestEnv::with_ledger_defaults(
            LedgerDefaults::new()
                .timestamp(1_700_000_000)
                .sequence_number(42_000)
                .protocol_version(current_protocol)
                .base_reserve(5_000_000),
        );
        let info = env.env().ledger().get();
        assert_eq!(env.now(), 1_700_000_000);
        assert_eq!(env.sequence(), 42_000);
        assert_eq!(info.protocol_version, current_protocol);
        assert_eq!(info.base_reserve, 5_000_000);
    }

    #[test]
    fn with_ledger_defaults_and_seed_is_deterministic() {
        let defaults = LedgerDefaults::new()
            .timestamp(500_000)
            .sequence_number(100);
        let a = TestEnv::with_ledger_defaults_and_seed(defaults.clone(), 7);
        let b = TestEnv::with_ledger_defaults_and_seed(defaults, 7);
        assert_eq!(a.now(), b.now());
        assert_eq!(a.sequence(), b.sequence());
        assert_eq!(a.address(), b.address());
    }

    #[test]
    fn ledger_defaults_are_carried_by_clone_config() {
        let defaults = LedgerDefaults::new()
            .timestamp(1_000_000)
            .sequence_number(500);
        let original = TestEnv::with_ledger_defaults(defaults);
        original.advance_ledgers(10);

        let clone = original.clone_config();
        // The clone starts from the defaults again, not from the original's
        // current position.
        assert_eq!(clone.now(), 1_000_000);
        assert_eq!(clone.sequence(), 500);
    }

    #[test]
    fn ledger_defaults_are_respected_after_reset() {
        let defaults = LedgerDefaults::new()
            .timestamp(2_000_000)
            .sequence_number(300);
        let mut env = TestEnv::with_ledger_defaults(defaults);
        env.advance_ledgers(50);
        assert_ne!(env.now(), 2_000_000);

        env.reset();
        assert_eq!(env.now(), 2_000_000);
        assert_eq!(env.sequence(), 300);
    }

    #[test]
    fn ledger_defaults_accessor_returns_what_was_given() {
        let defaults = LedgerDefaults::new().timestamp(99).sequence_number(7);
        let env = TestEnv::with_ledger_defaults(defaults);
        assert_eq!(env.ledger_defaults().timestamp_override(), Some(99));
        assert_eq!(env.ledger_defaults().sequence_number_override(), Some(7));
    }

    #[test]
    fn ledger_defaults_with_no_fields_is_same_as_new() {
        let with_defaults = TestEnv::with_ledger_defaults(LedgerDefaults::new());
        let plain = TestEnv::new();
        // Starting positions should be the same (both start at 0/0).
        assert_eq!(with_defaults.now(), plain.now());
        assert_eq!(with_defaults.sequence(), plain.sequence());
    }

    // ─────────────────────────────────────────────────────────────────────
    // Issue #39 — EnvMetadata in diagnostic output
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn metadata_captures_current_timestamp_and_sequence() {
        let env = TestEnv::new();
        env.advance_ledgers(10);
        let meta = env.metadata();
        assert_eq!(meta.timestamp, env.now());
        assert_eq!(meta.sequence, env.sequence());
    }

    #[test]
    fn metadata_captures_seed() {
        let env = TestEnv::with_seed(42);
        assert_eq!(env.metadata().seed, 42);
    }

    #[test]
    fn metadata_captures_close_interval_override() {
        let env = TestEnv::new().with_ledger_close_interval(3);
        assert_eq!(env.metadata().close_interval_override, Some(3));
    }

    #[test]
    fn metadata_has_no_close_interval_when_none_was_set() {
        let env = TestEnv::new();
        assert_eq!(env.metadata().close_interval_override, None);
    }

    #[test]
    fn metadata_display_includes_all_key_fields() {
        let env = TestEnv::with_seed(7).with_ledger_close_interval(2);
        env.advance_ledgers(5);
        let display = env.metadata().to_string();
        assert!(display.contains("seed=7"), "{display}");
        assert!(display.contains("close_interval=2s"), "{display}");
        // sequence and timestamp both have non-zero values now
        assert!(display.contains("sequence="), "{display}");
        assert!(display.contains("timestamp="), "{display}");
    }

    #[test]
    fn metadata_display_omits_close_interval_when_not_set() {
        let env = TestEnv::with_seed(5);
        let display = env.metadata().to_string();
        assert!(!display.contains("close_interval"), "{display}");
    }

    #[test]
    fn metadata_updates_after_advancing_the_clock() {
        let env = TestEnv::new();
        let before = env.metadata();
        env.advance_ledgers(20);
        let after = env.metadata();
        assert!(after.timestamp > before.timestamp);
        assert!(after.sequence > before.sequence);
    }

    #[test]
    fn metadata_protocol_version_matches_ledger_info() {
        // Use the SDK's current protocol version to avoid the "too old" check.
        let current_protocol = soroban_sdk::Env::default().ledger().get().protocol_version;
        let env =
            TestEnv::with_ledger_defaults(LedgerDefaults::new().protocol_version(current_protocol));
        assert_eq!(env.metadata().protocol_version, current_protocol);
    }
} // end mod tests
