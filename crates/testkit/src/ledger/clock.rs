use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

use soroban_sdk::testutils::Ledger as _;

use crate::core::{TestEnv, TestkitError};

/// Stellar's current observed average ledger close time, in seconds.
///
/// This is **not** a protocol-guaranteed constant — it is a network
/// characteristic that has moved before (historically as high as ~5-6s,
/// with the Stellar Development Foundation targeting 2.5s in a future
/// release) and could move again. It is the default close interval every
/// [`TestEnv`] starts with; [`TestEnv::advance`] and
/// [`TestEnv::advance_ledgers`] use the interval only to keep `timestamp`
/// and `sequence` moving in proportion to each other, and no assertion in
/// this crate depends on its exact value. If the network average changes
/// meaningfully, update this constant — or, for a single test or fixture,
/// override it with [`TestEnv::with_ledger_close_interval`] — rather than
/// working around it at call sites.
pub const LEDGER_CLOSE_TIME_SECS: u64 = 5;

const SECS_PER_MINUTE: u64 = 60;
const SECS_PER_HOUR: u64 = 60 * SECS_PER_MINUTE;
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;

impl TestEnv {
    /// Configure how many seconds each ledger takes to close in this
    /// environment, returning the environment so fixtures can chain it onto
    /// construction.
    ///
    /// The default is [`LEDGER_CLOSE_TIME_SECS`]. The interval sets how
    /// `timestamp` and `sequence` move together: [`TestEnv::advance`] (and
    /// [`TestEnv::warp_to`] and the calendar helpers built on it) add
    /// `ceil(seconds / interval)` ledgers, and [`TestEnv::advance_ledgers`]
    /// adds `n * interval` seconds. The setting is per environment — other
    /// `TestEnv`s are unaffected — and configuring it does not move the
    /// clock.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if `secs` is zero: a zero
    /// interval cannot map elapsed time onto ledger sequence numbers.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// // A network that closes a ledger every 2 seconds.
    /// let env = TestEnv::new().with_ledger_close_interval(2);
    /// let (now, sequence) = (env.now(), env.sequence());
    /// env.advance_ledgers(10);
    /// assert_eq!(env.now(), now + 20);
    /// assert_eq!(env.sequence(), sequence + 10);
    /// ```
    pub fn with_ledger_close_interval(mut self, secs: u64) -> Self {
        if secs == 0 {
            panic!(
                "{}",
                TestkitError::Misuse(
                    "ledger close interval must be at least 1 second; a zero interval \
                     cannot map elapsed time onto ledger sequence numbers"
                        .into()
                )
            );
        }
        self.set_close_interval_override(secs);
        self
    }

    /// The number of seconds each ledger takes to close in this
    /// environment: [`LEDGER_CLOSE_TIME_SECS`] unless overridden with
    /// [`TestEnv::with_ledger_close_interval`].
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    /// use soroban_testkit::ledger::LEDGER_CLOSE_TIME_SECS;
    ///
    /// assert_eq!(TestEnv::new().ledger_close_interval(), LEDGER_CLOSE_TIME_SECS);
    /// assert_eq!(
    ///     TestEnv::new().with_ledger_close_interval(2).ledger_close_interval(),
    ///     2
    /// );
    /// ```
    pub fn ledger_close_interval(&self) -> u64 {
        self.close_interval_override()
            .unwrap_or(LEDGER_CLOSE_TIME_SECS)
    }

    /// Advance the ledger clock by a wall-clock duration, advancing the
    /// ledger sequence proportionally (per [`TestEnv::ledger_close_interval`]).
    ///
    /// Only whole seconds are applied; any sub-second part of `duration` is
    /// ignored, because ledger timestamps are whole unix seconds.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if the advance would overflow
    /// ledger arithmetic — the matching ledger count does not fit the
    /// `u32` sequence number, or the new sequence number or timestamp would
    /// pass its type's maximum. The clock is left unchanged.
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
        self.advance_secs(duration.as_secs());
    }

    /// Advance the ledger clock by `minutes` minutes (60 seconds each).
    ///
    /// Minutes are fixed-length: this is plain unix-time arithmetic with no
    /// leap seconds or time zones, matching how ledger timestamps work. See
    /// [`TestEnv::advance`] for how the sequence follows.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if `minutes` overflows a
    /// `u64` number of seconds, or if the advance would overflow ledger
    /// arithmetic (see [`TestEnv::advance`]). The clock is left unchanged.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let before = env.now();
    /// env.advance_minutes(5);
    /// assert_eq!(env.now(), before + 300);
    /// ```
    pub fn advance_minutes(&self, minutes: u64) {
        self.advance_secs(calendar_secs("advance_minutes", minutes, SECS_PER_MINUTE));
    }

    /// Advance the ledger clock by `hours` hours (3,600 seconds each).
    ///
    /// Hours are fixed-length: this is plain unix-time arithmetic with no
    /// leap seconds or time zones, matching how ledger timestamps work. See
    /// [`TestEnv::advance`] for how the sequence follows.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if `hours` overflows a `u64`
    /// number of seconds, or if the advance would overflow ledger arithmetic
    /// (see [`TestEnv::advance`]). The clock is left unchanged.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let before = env.now();
    /// env.advance_hours(2);
    /// assert_eq!(env.now(), before + 7_200);
    /// ```
    pub fn advance_hours(&self, hours: u64) {
        self.advance_secs(calendar_secs("advance_hours", hours, SECS_PER_HOUR));
    }

    /// Advance the ledger clock by `days` days (86,400 seconds each).
    ///
    /// Days are fixed-length: this is plain unix-time arithmetic with no
    /// leap seconds, daylight-saving shifts, or time zones, matching how
    /// ledger timestamps work. See [`TestEnv::advance`] for how the sequence
    /// follows.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if `days` overflows a `u64`
    /// number of seconds, or if the advance would overflow ledger arithmetic
    /// (see [`TestEnv::advance`]). The clock is left unchanged.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestEnv;
    ///
    /// let env = TestEnv::new();
    /// let before = env.now();
    /// env.advance_days(30);
    /// assert_eq!(env.now(), before + 30 * 86_400);
    /// ```
    pub fn advance_days(&self, days: u64) {
        self.advance_secs(calendar_secs("advance_days", days, SECS_PER_DAY));
    }

    /// Advance by exactly `n` ledgers, moving the timestamp forward by
    /// `n * `[`TestEnv::ledger_close_interval`].
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
                .saturating_add((n as u64).saturating_mul(self.ledger_close_interval())),
        );
    }

    /// Jump to an absolute unix timestamp, advancing the sequence in
    /// proportion to the elapsed time.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::Misuse`] if `timestamp` is before the
    /// environment's current time — the ledger clock cannot run backwards —
    /// or if the jump would overflow ledger arithmetic (see
    /// [`TestEnv::advance`]).
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

    /// Shared implementation of every seconds-based `advance*` helper: move
    /// the timestamp forward by `secs` and the sequence by the matching
    /// ledger count for this environment's close interval.
    ///
    /// Both new values are computed with overflow checks before either is
    /// written, so a rejected advance leaves the clock untouched.
    fn advance_secs(&self, secs: u64) {
        let info = self.env().ledger().get();
        let interval = self.ledger_close_interval();
        let ledgers = secs.div_ceil(interval);
        let target = u32::try_from(ledgers)
            .ok()
            .and_then(|ledgers| info.sequence_number.checked_add(ledgers))
            .zip(info.timestamp.checked_add(secs));
        let Some((sequence, timestamp)) = target else {
            panic!(
                "{}",
                TestkitError::Misuse(format!(
                    "advancing the ledger clock by {secs}s overflows ledger arithmetic: \
                     at {interval}s per ledger that is {ledgers} ledgers, but the clock is \
                     at sequence {} (a u32) and timestamp {} (a u64); use a shorter duration",
                    info.sequence_number, info.timestamp
                ))
            );
        };
        self.env().ledger().set_timestamp(timestamp);
        self.env().ledger().set_sequence_number(sequence);
    }
}

/// Convert `count` calendar units of `secs_per_unit` seconds into seconds,
/// panicking with a [`TestkitError::Misuse`] naming `helper` if the product
/// does not fit in a `u64`.
fn calendar_secs(helper: &str, count: u64, secs_per_unit: u64) -> u64 {
    count.checked_mul(secs_per_unit).unwrap_or_else(|| {
        panic!(
            "{}",
            TestkitError::Misuse(format!(
                "{helper}({count}) overflows ledger arithmetic: \
                 {count} * {secs_per_unit}s does not fit in a u64 number of seconds"
            ))
        )
    })
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

    // --- advance overflow rejection -------------------------------------

    // Regression: `ledgers as u32` used to truncate, so a duration worth
    // exactly 2^32 ledgers advanced the timestamp by ~680 years while
    // leaving the sequence number where it was.
    #[test]
    #[should_panic(expected = "overflows ledger arithmetic")]
    fn advance_rejects_a_ledger_count_that_wraps_u32_to_zero() {
        let env = TestEnv::new();
        let wrapping_secs = (u64::from(u32::MAX) + 1) * LEDGER_CLOSE_TIME_SECS;
        env.advance(Duration::from_secs(wrapping_secs));
    }

    #[test]
    #[should_panic(expected = "overflows ledger arithmetic")]
    fn advance_rejects_the_maximum_duration() {
        let env = TestEnv::new();
        env.advance(Duration::from_secs(u64::MAX));
    }

    #[test]
    #[should_panic(expected = "overflows ledger arithmetic")]
    fn advance_rejects_sequence_overflow_instead_of_saturating() {
        let env = TestEnv::new();
        env.advance_ledgers(u32::MAX);
        env.advance(Duration::from_secs(LEDGER_CLOSE_TIME_SECS));
    }

    #[test]
    #[should_panic(expected = "overflows ledger arithmetic")]
    fn advance_rejects_timestamp_overflow_instead_of_saturating() {
        let env = TestEnv::new();
        soroban_sdk::testutils::Ledger::set_timestamp(&env.env().ledger(), u64::MAX - 10);
        env.advance(Duration::from_secs(60));
    }

    #[test]
    fn advance_accepts_a_duration_landing_exactly_on_the_sequence_limit() {
        let env = TestEnv::new();
        let room = u64::from(u32::MAX - env.sequence());
        env.advance(Duration::from_secs(room * LEDGER_CLOSE_TIME_SECS));
        assert_eq!(env.sequence(), u32::MAX);
    }

    #[test]
    #[should_panic(expected = "overflows ledger arithmetic")]
    fn advance_rejects_a_duration_one_second_past_the_sequence_limit() {
        let env = TestEnv::new();
        let room = u64::from(u32::MAX - env.sequence());
        env.advance(Duration::from_secs(room * LEDGER_CLOSE_TIME_SECS + 1));
    }

    #[test]
    fn rejected_advance_leaves_the_clock_untouched() {
        let env = TestEnv::new();
        env.advance(Duration::from_secs(100));
        let (now, sequence) = (env.now(), env.sequence());
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            env.advance(Duration::from_secs(u64::MAX));
        }));
        assert!(result.is_err());
        assert_eq!(env.now(), now);
        assert_eq!(env.sequence(), sequence);
    }

    #[test]
    fn overflow_message_names_the_offending_duration() {
        let env = TestEnv::new();
        let payload = panic::catch_unwind(AssertUnwindSafe(|| {
            env.advance(Duration::from_secs(u64::MAX));
        }))
        .unwrap_err();
        let message = payload
            .downcast_ref::<String>()
            .expect("panic payload should be a formatted String");
        assert!(message.contains("misuse of testkit API"), "{message}");
        assert!(message.contains(&u64::MAX.to_string()), "{message}");
    }

    #[test]
    #[should_panic(expected = "overflows ledger arithmetic")]
    fn warp_to_rejects_a_jump_that_overflows_ledger_arithmetic() {
        let env = TestEnv::new();
        env.warp_to(u64::MAX);
    }

    // --- configurable close interval ------------------------------------

    #[test]
    fn default_close_interval_is_the_crate_constant() {
        assert_eq!(
            TestEnv::new().ledger_close_interval(),
            LEDGER_CLOSE_TIME_SECS
        );
    }

    #[test]
    fn custom_interval_changes_how_advance_maps_time_to_ledgers() {
        let env = TestEnv::new().with_ledger_close_interval(2);
        let (now, sequence) = (env.now(), env.sequence());
        env.advance(Duration::from_secs(20));
        assert_eq!(env.now(), now + 20);
        assert_eq!(env.sequence(), sequence + 10);
    }

    #[test]
    fn custom_interval_changes_how_advance_ledgers_maps_ledgers_to_time() {
        let env = TestEnv::new().with_ledger_close_interval(2);
        let (now, sequence) = (env.now(), env.sequence());
        env.advance_ledgers(10);
        assert_eq!(env.sequence(), sequence + 10);
        assert_eq!(env.now(), now + 20);
    }

    #[test]
    fn custom_interval_applies_to_warp_to() {
        let env = TestEnv::new().with_ledger_close_interval(10);
        let sequence = env.sequence();
        env.warp_to(100);
        assert_eq!(env.now(), 100);
        assert_eq!(env.sequence(), sequence + 10);
    }

    #[test]
    fn advance_rounds_up_to_a_whole_ledger_for_a_custom_interval() {
        let env = TestEnv::new().with_ledger_close_interval(3);
        let sequence = env.sequence();
        env.advance(Duration::from_secs(7));
        assert_eq!(env.sequence(), sequence + 3);
    }

    #[test]
    fn advance_by_zero_moves_nothing_for_any_interval() {
        let env = TestEnv::new().with_ledger_close_interval(1);
        let (now, sequence) = (env.now(), env.sequence());
        env.advance(Duration::ZERO);
        assert_eq!((env.now(), env.sequence()), (now, sequence));
    }

    #[test]
    fn a_one_second_interval_maps_seconds_to_ledgers_one_to_one() {
        let env = TestEnv::new().with_ledger_close_interval(1);
        let sequence = env.sequence();
        env.advance(Duration::from_secs(42));
        assert_eq!(env.sequence(), sequence + 42);
    }

    #[test]
    fn a_huge_interval_still_advances_at_least_one_ledger() {
        let env = TestEnv::new().with_ledger_close_interval(u64::MAX);
        let sequence = env.sequence();
        env.advance(Duration::from_secs(60));
        assert_eq!(env.sequence(), sequence + 1);
    }

    #[test]
    fn close_interval_is_per_environment() {
        let fast = TestEnv::new().with_ledger_close_interval(1);
        let default = TestEnv::new();
        assert_eq!(fast.ledger_close_interval(), 1);
        assert_eq!(default.ledger_close_interval(), LEDGER_CLOSE_TIME_SECS);
    }

    #[test]
    fn configuring_the_interval_does_not_move_the_clock_or_change_the_seed() {
        let plain = TestEnv::with_seed(7);
        let (now, sequence) = (plain.now(), plain.sequence());
        let configured = TestEnv::with_seed(7).with_ledger_close_interval(2);
        assert_eq!((configured.now(), configured.sequence()), (now, sequence));
        assert_eq!(configured.seed(), 7);
    }

    #[test]
    #[should_panic(expected = "ledger close interval must be at least 1 second")]
    fn zero_close_interval_is_rejected() {
        let _ = TestEnv::new().with_ledger_close_interval(0);
    }

    // --- calendar helpers -----------------------------------------------

    #[test]
    fn advance_minutes_moves_timestamp_by_sixty_seconds_each() {
        let env = TestEnv::new();
        let before = env.now();
        env.advance_minutes(3);
        assert_eq!(env.now(), before + 180);
    }

    #[test]
    fn advance_hours_moves_timestamp_by_3600_seconds_each() {
        let env = TestEnv::new();
        let before = env.now();
        env.advance_hours(2);
        assert_eq!(env.now(), before + 7_200);
    }

    #[test]
    fn advance_days_moves_timestamp_and_sequence_together() {
        let env = TestEnv::new();
        let (now, sequence) = (env.now(), env.sequence());
        env.advance_days(1);
        assert_eq!(env.now(), now + 86_400);
        assert_eq!(
            env.sequence(),
            sequence + (86_400 / LEDGER_CLOSE_TIME_SECS) as u32
        );
    }

    #[test]
    fn calendar_helpers_agree_with_each_other_and_with_advance() {
        let by_minutes = TestEnv::new();
        let by_hours = TestEnv::new();
        let by_days = TestEnv::new();
        let by_duration = TestEnv::new();
        by_minutes.advance_minutes(24 * 60);
        by_hours.advance_hours(24);
        by_days.advance_days(1);
        by_duration.advance(Duration::from_secs(86_400));
        for env in [&by_minutes, &by_hours, &by_days] {
            assert_eq!(env.now(), by_duration.now());
            assert_eq!(env.sequence(), by_duration.sequence());
        }
    }

    #[test]
    fn calendar_helpers_accept_zero_and_move_nothing() {
        let env = TestEnv::new();
        let (now, sequence) = (env.now(), env.sequence());
        env.advance_minutes(0);
        env.advance_hours(0);
        env.advance_days(0);
        assert_eq!((env.now(), env.sequence()), (now, sequence));
    }

    #[test]
    fn calendar_helpers_respect_a_custom_close_interval() {
        let env = TestEnv::new().with_ledger_close_interval(60);
        let sequence = env.sequence();
        env.advance_hours(1);
        assert_eq!(env.sequence(), sequence + 60);
    }

    #[test]
    fn calendar_helpers_accumulate() {
        let env = TestEnv::new();
        let before = env.now();
        env.advance_days(1);
        env.advance_hours(1);
        env.advance_minutes(1);
        assert_eq!(env.now(), before + 86_400 + 3_600 + 60);
    }

    #[test]
    #[should_panic(expected = "advance_minutes(18446744073709551615) overflows ledger arithmetic")]
    fn advance_minutes_rejects_a_count_that_overflows_seconds() {
        TestEnv::new().advance_minutes(u64::MAX);
    }

    #[test]
    #[should_panic(expected = "advance_hours(18446744073709551615) overflows ledger arithmetic")]
    fn advance_hours_rejects_a_count_that_overflows_seconds() {
        TestEnv::new().advance_hours(u64::MAX);
    }

    #[test]
    #[should_panic(expected = "advance_days(18446744073709551615) overflows ledger arithmetic")]
    fn advance_days_rejects_a_count_that_overflows_seconds() {
        TestEnv::new().advance_days(u64::MAX);
    }

    #[test]
    #[should_panic(expected = "overflows ledger arithmetic")]
    fn advance_days_rejects_seconds_that_fit_u64_but_not_the_sequence() {
        TestEnv::new().advance_days(u64::MAX / SECS_PER_DAY);
    }

    #[test]
    fn rejected_calendar_advance_leaves_the_clock_untouched() {
        let env = TestEnv::new();
        env.advance_minutes(5);
        let (now, sequence) = (env.now(), env.sequence());
        let result = panic::catch_unwind(AssertUnwindSafe(|| env.advance_days(u64::MAX)));
        assert!(result.is_err());
        assert_eq!((env.now(), env.sequence()), (now, sequence));
    }
}
