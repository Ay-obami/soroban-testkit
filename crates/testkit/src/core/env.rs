use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

use crate::core::TestkitError;

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

/// Reusable test setup built around a [`TestEnv`].
///
/// Implement this trait for a fixture struct that owns a `TestEnv` plus the
/// addresses, contracts, or tokens a group of tests share. The provided
/// constructors keep environment creation consistent and make seeded fixtures
/// reproducible without duplicating setup boilerplate.
///
/// # Example
///
/// ```
/// use soroban_testkit::core::{TestEnv, TestFixture};
/// use soroban_sdk::Address;
///
/// struct Fixture {
///     env: TestEnv,
///     alice: Address,
/// }
///
/// impl TestFixture for Fixture {
///     fn from_env(env: TestEnv) -> Self {
///         let alice = env.address();
///         Self { env, alice }
///     }
///
///     fn test_env(&self) -> &TestEnv {
///         &self.env
///     }
/// }
///
/// let a = Fixture::with_seed(7);
/// let b = Fixture::with_seed(7);
/// assert_eq!(a.alice, b.alice);
/// ```
pub trait TestFixture: Sized {
    /// Build the fixture from an already-created test environment.
    fn from_env(env: TestEnv) -> Self;

    /// Borrow the environment owned by this fixture.
    fn test_env(&self) -> &TestEnv;

    /// Build the fixture around a fresh [`TestEnv`].
    fn new() -> Self {
        Self::from_env(TestEnv::new())
    }

    /// Build the fixture around a reproducibly seeded [`TestEnv`].
    fn with_seed(seed: u64) -> Self {
        Self::from_env(TestEnv::with_seed(seed))
    }
}

impl TestFixture for TestEnv {
    fn from_env(env: TestEnv) -> Self {
        env
    }

    fn test_env(&self) -> &TestEnv {
        self
    }
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

    /// Return an infinite iterator yielding fresh, distinct [`Address`] values.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let addrs: Vec<_> = env.address_iter().take(4).collect();
    /// assert_eq!(addrs.len(), 4);
    /// ```
    pub fn address_iter(&self) -> AddressIter<'_> {
        AddressIter::new(self)
    }

    /// Plural alias for [`TestEnv::address_iter`].
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let addrs: Vec<_> = env.addresses_iter().take(2).collect();
    /// assert_eq!(addrs.len(), 2);
    /// ```
    pub fn addresses_iter(&self) -> AddressIter<'_> {
        self.address_iter()
    }

    /// Generate a batch of `n` fresh addresses, returning `Err(TestkitError::Misuse)`
    /// if `n == 0` or if `n` exceeds [`MAX_ADDRESS_BATCH_SIZE`].
    ///
    /// # Errors
    ///
    /// Returns a [`TestkitError::Misuse`] if:
    /// - `n == 0`: requesting zero addresses indicates a test setup error.
    /// - `n > MAX_ADDRESS_BATCH_SIZE`: batch size exceeds the allowed threshold.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let addrs = env.try_addresses(3).expect("valid batch size");
    /// assert_eq!(addrs.len(), 3);
    ///
    /// let err = env.try_addresses(0).unwrap_err();
    /// assert_eq!(err.code(), "TESTKIT_MISUSE");
    /// ```
    pub fn try_addresses(&self, n: usize) -> Result<Vec<Address>, TestkitError> {
        if n == 0 {
            return Err(TestkitError::Misuse(
                "address batch size must be at least 1; requested 0 addresses".into(),
            ));
        }
        if n > MAX_ADDRESS_BATCH_SIZE {
            return Err(TestkitError::Misuse(format!(
                "requested address batch size {n} exceeds the maximum limit of {MAX_ADDRESS_BATCH_SIZE}"
            )));
        }
        Ok(self.addresses(n))
    }

    /// Checked variant of address batch generation, equivalent to [`TestEnv::try_addresses`].
    ///
    /// # Errors
    ///
    /// Returns [`TestkitError::Misuse`] if `n == 0` or `n > MAX_ADDRESS_BATCH_SIZE`.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// assert!(env.checked_addresses(2).is_ok());
    /// assert!(env.checked_addresses(0).is_err());
    /// ```
    pub fn checked_addresses(&self, n: usize) -> Result<Vec<Address>, TestkitError> {
        self.try_addresses(n)
    }

    /// Generate a named [`Actor`] pairing the given identifier with a fresh [`Address`].
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if `name` is empty or consists solely of whitespace.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let alice = env.actor("alice");
    /// assert_eq!(alice.name(), "alice");
    /// ```
    pub fn actor(&self, name: &str) -> Actor {
        self.try_actor(name).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Checked variant of [`TestEnv::actor`], returning [`TestkitError::Misuse`]
    /// if `name` is empty or only whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`TestkitError::Misuse`] if `name` is empty or whitespace-only.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// assert!(env.try_actor("alice").is_ok());
    /// assert!(env.try_actor("").is_err());
    /// ```
    pub fn try_actor(&self, name: &str) -> Result<Actor, TestkitError> {
        if name.trim().is_empty() {
            return Err(TestkitError::Misuse(
                "actor name cannot be empty or only whitespace".into(),
            ));
        }
        Ok(Actor::new(name, self.address()))
    }

    /// Alias for [`TestEnv::actor`].
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let bob = env.named_actor("bob");
    /// assert_eq!(bob.name(), "bob");
    /// ```
    pub fn named_actor(&self, name: &str) -> Actor {
        self.actor(name)
    }

    /// Alias for [`TestEnv::try_actor`].
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// assert!(env.try_named_actor("bob").is_ok());
    /// assert!(env.try_named_actor("   ").is_err());
    /// ```
    pub fn try_named_actor(&self, name: &str) -> Result<Actor, TestkitError> {
        self.try_actor(name)
    }

    /// Generate multiple named [`Actor`]s from a slice of names.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if any name is empty/whitespace,
    /// or if duplicate names are specified.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let actors = env.actors(&["alice", "bob", "charlie"]);
    /// assert_eq!(actors.len(), 3);
    /// assert_ne!(actors[0].address(), actors[1].address());
    /// ```
    pub fn actors(&self, names: &[&str]) -> Vec<Actor> {
        self.try_actors(names).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Checked variant of [`TestEnv::actors`].
    ///
    /// # Errors
    ///
    /// Returns [`TestkitError::Misuse`] if any name is blank or if duplicate
    /// actor names are detected in `names`.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// assert!(env.try_actors(&["alice", "bob"]).is_ok());
    /// assert!(env.try_actors(&["alice", "alice"]).is_err());
    /// ```
    pub fn try_actors(&self, names: &[&str]) -> Result<Vec<Actor>, TestkitError> {
        for (i, &name) in names.iter().enumerate() {
            if name.trim().is_empty() {
                return Err(TestkitError::Misuse(
                    "actor name cannot be empty or only whitespace".into(),
                ));
            }
            for &earlier in &names[..i] {
                if earlier == name {
                    return Err(TestkitError::Misuse(format!(
                        "duplicate actor name {name:?} in batch request"
                    )));
                }
            }
        }
        Ok(names
            .iter()
            .map(|&n| Actor::new(n, self.address()))
            .collect())
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

/// Maximum number of addresses that can be requested in a single batch.
pub const MAX_ADDRESS_BATCH_SIZE: usize = 10_000;

/// A named test actor pairing a human-readable identifier (such as `"alice"` or
/// `"treasury"`) with a generated Soroban [`Address`].
///
/// In contract tests, raw cryptographic addresses (for example,
/// `Address(Account(GA...))`) in assertion failures or diagnostic logs make it
/// tedious to track which participant caused a failure. `Actor` bundles the
/// display name alongside the address so test diagnostics, auth matrices, and
/// logs clearly show the actor responsible.
///
/// `Actor` implements [`std::ops::Deref`] targeting [`Address`], so an `&Actor`
/// can be passed directly to any function expecting `&Address`. It also
/// implements [`std::fmt::Display`] for compact name rendering, and provides
/// [`Actor::diagnostic`] for formatting both name and raw address together.
///
/// # Example
///
/// ```
/// use soroban_testkit::core::TestEnv;
///
/// let env = TestEnv::new();
/// let alice = env.actor("alice");
///
/// assert_eq!(alice.name(), "alice");
/// assert_eq!(format!("{alice}"), "alice");
/// assert!(alice.diagnostic().starts_with("alice ("));
///
/// // Deref to Address works seamlessly:
/// let addr: &soroban_sdk::Address = &alice;
/// assert_eq!(addr, alice.address());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    name: String,
    address: Address,
}

impl Actor {
    /// Construct a new `Actor` with the given name and address.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{Actor, TestEnv};
    ///
    /// let env = TestEnv::new();
    /// let actor = Actor::new("bob", env.address());
    /// assert_eq!(actor.name(), "bob");
    /// ```
    pub fn new(name: impl Into<String>, address: Address) -> Self {
        Self {
            name: name.into(),
            address,
        }
    }

    /// The human-readable name identifying this actor.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let admin = env.actor("admin");
    /// assert_eq!(admin.name(), "admin");
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The underlying Soroban [`Address`] assigned to this actor.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let alice = env.actor("alice");
    /// assert_eq!(alice.address(), &*alice);
    /// ```
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Consume the actor, returning its inner [`Address`].
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let alice = env.actor("alice");
    /// let _addr = alice.into_address();
    /// ```
    pub fn into_address(self) -> Address {
        self.address
    }

    /// Return a formatted diagnostic string pairing the actor name with its
    /// raw address, suitable for detailed assertion failure reports.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let alice = env.actor("alice");
    /// assert!(alice.diagnostic().starts_with("alice ("));
    /// ```
    pub fn diagnostic(&self) -> String {
        format!("{} ({:?})", self.name, self.address)
    }
}

impl std::ops::Deref for Actor {
    type Target = Address;

    fn deref(&self) -> &Self::Target {
        &self.address
    }
}

impl AsRef<Address> for Actor {
    fn as_ref(&self) -> &Address {
        &self.address
    }
}

impl std::fmt::Display for Actor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl PartialEq<Address> for Actor {
    fn eq(&self, other: &Address) -> bool {
        &self.address == other
    }
}

impl PartialEq<Actor> for Address {
    fn eq(&self, other: &Actor) -> bool {
        self == &other.address
    }
}

/// An infinite iterator yielding fresh, distinct [`Address`] values on each step.
///
/// Standard iterator adapters such as [`.take(n)`](Iterator::take) can be chained
/// directly onto `AddressIter` to produce a stream of addresses without needing
/// to allocate an intermediate vector.
///
/// # Example
///
/// ```
/// use soroban_testkit::core::TestEnv;
///
/// let env = TestEnv::new();
/// let addrs: Vec<_> = env.address_iter().take(3).collect();
/// assert_eq!(addrs.len(), 3);
/// assert_ne!(addrs[0], addrs[1]);
/// ```
#[derive(Clone)]
pub struct AddressIter<'a> {
    env: &'a TestEnv,
}

impl<'a> AddressIter<'a> {
    /// Create a new `AddressIter` bound to the given environment.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::{AddressIter, TestEnv};
    ///
    /// let env = TestEnv::new();
    /// let mut iter = AddressIter::new(&env);
    /// let _first = iter.next().unwrap();
    /// ```
    pub fn new(env: &'a TestEnv) -> Self {
        Self { env }
    }
}

impl<'a> Iterator for AddressIter<'a> {
    type Item = Address;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.env.address())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

impl<'a> std::iter::FusedIterator for AddressIter<'a> {}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Ledger;

    struct Fixture {
        env: TestEnv,
        first: Address,
    }

    impl TestFixture for Fixture {
        fn from_env(env: TestEnv) -> Self {
            let first = env.address();
            Self { env, first }
        }

        fn test_env(&self) -> &TestEnv {
            &self.env
        }
    }

    #[test]
    fn fixture_with_seed_is_reproducible() {
        let a = Fixture::with_seed(42);
        let b = Fixture::with_seed(42);
        assert_eq!(a.first, b.first);
        assert_eq!(a.test_env().seed(), 42);
        assert_eq!(b.test_env().seed(), 42);
    }

    #[test]
    fn fixture_new_environments_are_isolated() {
        let a = Fixture::new();
        let b = Fixture::new();
        a.test_env().env().ledger().set_sequence_number(9_999);
        assert_ne!(a.test_env().sequence(), b.test_env().sequence());
    }

    #[test]
    fn fixture_accepts_zero_seed_and_existing_env_configuration() {
        let zero = Fixture::with_seed(0);
        assert_eq!(zero.test_env().seed(), 0);

        let configured = Fixture::from_env(TestEnv::with_seed(7).with_ledger_close_interval(2));
        assert_eq!(configured.test_env().seed(), 7);
        assert_eq!(configured.test_env().ledger_close_interval(), 2);
    }

    #[test]
    fn test_env_itself_implements_fixture() {
        let env = <TestEnv as TestFixture>::with_seed(11);
        assert_eq!(env.seed(), 11);
        assert!(std::ptr::eq(<TestEnv as TestFixture>::test_env(&env), &env));
    }

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

    // --- Named actor generation (#36) -----------------------------------

    #[test]
    fn actor_creates_actor_with_expected_name_and_address() {
        let env = TestEnv::new();
        let alice = env.actor("alice");
        assert_eq!(alice.name(), "alice");
        assert_eq!(alice.address(), &*alice);
        assert_eq!(format!("{alice}"), "alice");
    }

    #[test]
    fn actor_diagnostic_formats_name_and_debug_address() {
        let env = TestEnv::new();
        let alice = env.actor("alice");
        let diag = alice.diagnostic();
        assert!(diag.starts_with("alice ("));
        assert!(diag.ends_with(')'));
    }

    #[test]
    fn actor_derefs_and_compares_with_address() {
        let env = TestEnv::new();
        let alice = env.actor("alice");
        let raw_addr: &Address = &alice;
        assert_eq!(raw_addr, alice.address());
        assert_eq!(alice, *alice.address());
        assert_eq!(*alice.address(), alice);

        let bob = env.actor("bob");
        assert_ne!(alice, bob);
    }

    #[test]
    fn actor_into_address_returns_underlying_address() {
        let env = TestEnv::new();
        let alice = env.actor("alice");
        let expected = alice.address().clone();
        assert_eq!(alice.into_address(), expected);
    }

    #[test]
    fn try_actor_accepts_valid_names() {
        let env = TestEnv::new();
        assert!(env.try_actor("alice").is_ok());
        assert!(env.try_actor("treasury_1").is_ok());
        assert!(env.try_named_actor("admin").is_ok());
    }

    #[test]
    fn try_actor_rejects_empty_or_whitespace_names() {
        let env = TestEnv::new();
        let err_empty = env.try_actor("").unwrap_err();
        assert_eq!(err_empty.code(), "TESTKIT_MISUSE");
        assert_eq!(
            err_empty.message(),
            "actor name cannot be empty or only whitespace"
        );

        let err_space = env.try_actor("   \t\n").unwrap_err();
        assert_eq!(err_space.code(), "TESTKIT_MISUSE");
    }

    #[test]
    #[should_panic(expected = "actor name cannot be empty or only whitespace")]
    fn actor_panics_on_empty_name() {
        let env = TestEnv::new();
        let _ = env.actor("");
    }

    #[test]
    #[should_panic(expected = "actor name cannot be empty or only whitespace")]
    fn named_actor_panics_on_whitespace_name() {
        let env = TestEnv::new();
        let _ = env.named_actor("   ");
    }

    #[test]
    fn actors_batch_generates_distinct_actors() {
        let env = TestEnv::new();
        let actors = env.actors(&["alice", "bob", "charlie"]);
        assert_eq!(actors.len(), 3);
        assert_eq!(actors[0].name(), "alice");
        assert_eq!(actors[1].name(), "bob");
        assert_eq!(actors[2].name(), "charlie");
        assert_ne!(actors[0].address(), actors[1].address());
        assert_ne!(actors[1].address(), actors[2].address());
        assert_ne!(actors[0].address(), actors[2].address());
    }

    #[test]
    fn try_actors_rejects_duplicate_names() {
        let env = TestEnv::new();
        let err = env.try_actors(&["alice", "bob", "alice"]).unwrap_err();
        assert_eq!(err.code(), "TESTKIT_MISUSE");
        assert!(err.message().contains("duplicate actor name \"alice\""));
    }

    #[test]
    fn try_actors_rejects_blank_name() {
        let env = TestEnv::new();
        let err = env.try_actors(&["alice", ""]).unwrap_err();
        assert_eq!(err.code(), "TESTKIT_MISUSE");
    }

    #[test]
    #[should_panic(expected = "duplicate actor name")]
    fn actors_panics_on_duplicate_name() {
        let env = TestEnv::new();
        let _ = env.actors(&["admin", "admin"]);
    }

    // --- Address iterator (#37) -----------------------------------------

    #[test]
    fn address_iter_yields_fresh_distinct_addresses() {
        let env = TestEnv::new();
        let addrs: Vec<Address> = env.address_iter().take(5).collect();
        assert_eq!(addrs.len(), 5);
        for i in 0..addrs.len() {
            for j in (i + 1)..addrs.len() {
                assert_ne!(addrs[i], addrs[j]);
            }
        }
    }

    #[test]
    fn addresses_iter_alias_behaves_identically() {
        let env = TestEnv::new();
        let mut iter = env.addresses_iter();
        let a = iter.next().unwrap();
        let b = iter.next().unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn address_iter_reproducible_with_seed() {
        let env1 = TestEnv::with_seed(12345);
        let env2 = TestEnv::with_seed(12345);
        let seq1: Vec<Address> = env1.address_iter().take(4).collect();
        let seq2: Vec<Address> = env2.address_iter().take(4).collect();
        assert_eq!(seq1, seq2);
    }

    #[test]
    fn address_iter_size_hint_and_cloning() {
        let env = TestEnv::new();
        let iter = env.address_iter();
        let (lower, upper) = iter.size_hint();
        assert_eq!(lower, usize::MAX);
        assert_eq!(upper, None);

        let mut iter_clone = iter.clone();
        assert!(iter_clone.next().is_some());
    }

    // --- Checked address batch API (#38) --------------------------------

    #[test]
    fn try_addresses_succeeds_for_valid_batch_sizes() {
        let env = TestEnv::new();
        let one = env.try_addresses(1).unwrap();
        assert_eq!(one.len(), 1);

        let five = env.try_addresses(5).unwrap();
        assert_eq!(five.len(), 5);
        for i in 0..five.len() {
            for j in (i + 1)..five.len() {
                assert_ne!(five[i], five[j]);
            }
        }
    }

    #[test]
    fn checked_addresses_alias_succeeds() {
        let env = TestEnv::new();
        let res = env.checked_addresses(3);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 3);
    }

    #[test]
    fn try_addresses_rejects_zero_count() {
        let env = TestEnv::new();
        let err = env.try_addresses(0).unwrap_err();
        assert_eq!(err.code(), "TESTKIT_MISUSE");
        assert_eq!(
            err.message(),
            "address batch size must be at least 1; requested 0 addresses"
        );
    }

    #[test]
    fn try_addresses_rejects_excessive_batch_size() {
        let env = TestEnv::new();
        let err = env.try_addresses(MAX_ADDRESS_BATCH_SIZE + 1).unwrap_err();
        assert_eq!(err.code(), "TESTKIT_MISUSE");
        assert!(err.message().contains("exceeds the maximum limit"));

        let err_max = env.try_addresses(usize::MAX).unwrap_err();
        assert_eq!(err_max.code(), "TESTKIT_MISUSE");
    }

    #[test]
    fn try_addresses_reproducible_for_same_seed() {
        let env1 = TestEnv::with_seed(999);
        let env2 = TestEnv::with_seed(999);
        let batch1 = env1.try_addresses(4).unwrap();
        let batch2 = env2.try_addresses(4).unwrap();
        assert_eq!(batch1, batch2);
    }
}
