use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

use soroban_sdk::testutils::Ledger as _;

use crate::core::{TestEnv, TestkitError};

/// Stellar's current observed average ledger close time, in seconds.
///
/// This is **not** a protocol-guaranteed constant — it is a network
/// characteristic that has moved before (historically as high as ~5-6s,
/// with the Stellar Development Foundation targeting 2.5s in a future
/// release) and could move again. [`TestEnv::advance`] and
/// [`TestEnv::advance_ledgers`] use it only to keep `timestamp` and
/// `sequence` moving in proportion to each other; no assertion in this
/// crate depends on its exact value. If the network average changes
/// meaningfully, update this constant rather than working around it at
/// call sites.
pub const LEDGER_CLOSE_TIME_SECS: u64 = 5;

impl TestEnv {
    /// Advance the ledger clock by a wall-clock duration, advancing the
    /// ledger sequence proportionally (per [`LEDGER_CLOSE_TIME_SECS`]).
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    /// use std::time::Duration;
    ///
    /// let env = TestEnv::new();
    /// let before = env.now();
    /// env.advance(Duration::from_secs(60));
    /// assert_eq!(env.now(), before + 60);
    /// ```
    pub fn advance(&self, duration: Duration) {
        let secs = duration.as_secs();
        let ledgers = secs.div_ceil(LEDGER_CLOSE_TIME_SECS);
        let info = self.env().ledger().get();
        self.env()
            .ledger()
            .set_timestamp(info.timestamp.saturating_add(secs));
        self.env()
            .ledger()
            .set_sequence_number(info.sequence_number.saturating_add(ledgers as u32));
    }

    /// Advance by exactly `n` ledgers, moving the timestamp forward by
    /// `n * `[`LEDGER_CLOSE_TIME_SECS`].
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let before = env.sequence();
    /// env.advance_ledgers(10);
    /// assert_eq!(env.sequence(), before + 10);
    /// ```
    pub fn advance_ledgers(&self, n: u32) {
        let info = self.env().ledger().get();
        self.env()
            .ledger()
            .set_sequence_number(info.sequence_number.saturating_add(n));
        self.env().ledger().set_timestamp(
            info.timestamp
                .saturating_add((n as u64).saturating_mul(LEDGER_CLOSE_TIME_SECS)),
        );
    }

    /// Jump to an absolute unix timestamp, advancing the sequence in
    /// proportion to the elapsed time.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if `timestamp` is before the
    /// environment's current time — the ledger clock cannot run backwards.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// env.warp_to(env.now() + 3600);
    /// assert_eq!(env.now(), 3600);
    /// ```
    pub fn warp_to(&self, timestamp: u64) {
        let now = self.now();
        if timestamp < now {
            panic!(
                "{}",
                TestkitError::Misuse(format!(
                    "warp_to({timestamp}) is before the current ledger time ({now}); \
                     the ledger clock cannot run backwards"
                ))
            );
        }
        self.advance(Duration::from_secs(timestamp - now));
    }

    /// The current ledger timestamp (unix seconds).
    pub fn now(&self) -> u64 {
        self.env().ledger().timestamp()
    }

    /// The current ledger sequence number.
    pub fn sequence(&self) -> u32 {
        self.env().ledger().sequence()
    }

    /// Run `f` with the ledger clock set to `timestamp`, then restore the
    /// clock to whatever it was before — even if `f` panics.
    ///
    /// This is for evaluating time-dependent contract state (e.g. a vesting
    /// schedule) at several points in time without each call site having to
    /// save and restore the clock by hand.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let before = env.now();
    /// let observed = env.at(before + 1_000, || env.now());
    /// assert_eq!(observed, before + 1_000);
    /// assert_eq!(env.now(), before);
    /// ```
    pub fn at<T>(&self, timestamp: u64, f: impl FnOnce() -> T) -> T {
        let original = self.env().ledger().get();
        self.env().ledger().set_timestamp(timestamp);
        let result = panic::catch_unwind(AssertUnwindSafe(f));
        self.env().ledger().set(original);
        match result {
            Ok(value) => value,
            Err(payload) => panic::resume_unwind(payload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_moves_timestamp_by_exact_duration() {
        let env = TestEnv::new();
        let before = env.now();
        env.advance(Duration::from_secs(60));
        assert_eq!(env.now(), before + 60);
    }

    #[test]
    fn advance_moves_sequence_consistently_with_close_time() {
        let env = TestEnv::new();
        let before = env.sequence();
        env.advance(Duration::from_secs(LEDGER_CLOSE_TIME_SECS * 10));
        assert_eq!(env.sequence(), before + 10);
    }

    #[test]
    fn advance_ledgers_moves_sequence_by_exactly_n() {
        let env = TestEnv::new();
        let before = env.sequence();
        env.advance_ledgers(7);
        assert_eq!(env.sequence(), before + 7);
    }

    #[test]
    #[should_panic(expected = "warp_to")]
    fn warp_to_past_timestamp_panics() {
        let env = TestEnv::new();
        env.advance(Duration::from_secs(100));
        env.warp_to(0);
    }

    #[test]
    fn warp_to_future_timestamp_sets_now() {
        let env = TestEnv::new();
        env.warp_to(500);
        assert_eq!(env.now(), 500);
    }

    #[test]
    fn at_restores_clock_after_returning() {
        let env = TestEnv::new();
        let before = env.now();
        let seen = env.at(before + 42, || env.now());
        assert_eq!(seen, before + 42);
        assert_eq!(env.now(), before);
    }

    #[test]
    fn at_restores_clock_even_if_closure_panics() {
        let env = TestEnv::new();
        let before = env.now();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            env.at(before + 999, || panic!("boom"));
        }));
        assert!(result.is_err());
        assert_eq!(env.now(), before);
    }
}
