/// Errors raised by testkit assertion helpers and setup routines.
///
/// Assertion helpers throughout this crate panic deliberately when a
/// contract under test misbehaves — that panic is their contract with the
/// caller — but they always panic with a `TestkitError` as context rather
/// than a bare string, so the failure carries a stable, matchable message.
///
/// # Example
///
/// ```
/// use soroban_testkit::core::TestkitError;
///
/// let err = TestkitError::Misuse("events were never captured".into());
/// assert_eq!(
///     err.to_string(),
///     "misuse of testkit API: events were never captured"
/// );
/// ```
#[derive(Debug, thiserror::Error)]
pub enum TestkitError {
    /// An assertion helper's expectation about contract state or behavior
    /// was not met (for example, an expected event was never emitted, or a
    /// conservation invariant did not hold).
    #[error("assertion failed: {0}")]
    AssertionFailed(String),

    /// A captured value could not be decoded into the requested type.
    #[error("failed to decode value: {0}")]
    DecodeFailed(String),

    /// The testkit API was used in a way its contract does not allow (for
    /// example, asserting on events before capture was enabled, or warping
    /// the ledger clock backwards).
    #[error("misuse of testkit API: {0}")]
    Misuse(String),
}

impl TestkitError {
    /// A stable, machine-readable code identifying which kind of error this
    /// is, independent of the human-readable message.
    ///
    /// The message text is meant for people and may be reworded; the code is
    /// meant for tooling (log filters, CI annotations, `match` on a
    /// caught panic payload) and is part of this crate's compatibility
    /// promise: an existing variant's code never changes, and a code is
    /// never reused for a different variant. New variants get new codes.
    ///
    /// | Variant | Code |
    /// |---|---|
    /// | [`TestkitError::AssertionFailed`] | `TESTKIT_ASSERTION_FAILED` |
    /// | [`TestkitError::DecodeFailed`] | `TESTKIT_DECODE_FAILED` |
    /// | [`TestkitError::Misuse`] | `TESTKIT_MISUSE` |
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::core::TestkitError;
    ///
    /// let err = TestkitError::Misuse("events were never captured".into());
    /// assert_eq!(err.code(), "TESTKIT_MISUSE");
    /// ```
    pub fn code(&self) -> &'static str {
        match self {
            TestkitError::AssertionFailed(_) => "TESTKIT_ASSERTION_FAILED",
            TestkitError::DecodeFailed(_) => "TESTKIT_DECODE_FAILED",
            TestkitError::Misuse(_) => "TESTKIT_MISUSE",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assertion_failed_message() {
        let err = TestkitError::AssertionFailed("deposited != withdrawn".into());
        assert_eq!(err.to_string(), "assertion failed: deposited != withdrawn");
    }

    #[test]
    fn decode_failed_message() {
        let err = TestkitError::DecodeFailed("expected i128, got Symbol".into());
        assert_eq!(
            err.to_string(),
            "failed to decode value: expected i128, got Symbol"
        );
    }

    #[test]
    fn misuse_message() {
        let err = TestkitError::Misuse("events were never captured".into());
        assert_eq!(
            err.to_string(),
            "misuse of testkit API: events were never captured"
        );
    }

    // The literals below are the public contract: changing one is a
    // breaking change for anyone matching on codes.
    #[test]
    fn each_variant_has_its_documented_code() {
        assert_eq!(
            TestkitError::AssertionFailed("x".into()).code(),
            "TESTKIT_ASSERTION_FAILED"
        );
        assert_eq!(
            TestkitError::DecodeFailed("x".into()).code(),
            "TESTKIT_DECODE_FAILED"
        );
        assert_eq!(TestkitError::Misuse("x".into()).code(), "TESTKIT_MISUSE");
    }

    #[test]
    fn codes_are_unique_across_variants() {
        let codes = [
            TestkitError::AssertionFailed(String::new()).code(),
            TestkitError::DecodeFailed(String::new()).code(),
            TestkitError::Misuse(String::new()).code(),
        ];
        for i in 0..codes.len() {
            for j in (i + 1)..codes.len() {
                assert_ne!(codes[i], codes[j]);
            }
        }
    }

    #[test]
    fn code_does_not_depend_on_the_message() {
        assert_eq!(
            TestkitError::Misuse(String::new()).code(),
            TestkitError::Misuse("a much longer, different message".into()).code()
        );
    }

    #[test]
    fn codes_are_screaming_snake_case() {
        let errors = [
            TestkitError::AssertionFailed(String::new()),
            TestkitError::DecodeFailed(String::new()),
            TestkitError::Misuse(String::new()),
        ];
        for err in &errors {
            let code = err.code();
            assert!(!code.is_empty());
            assert!(
                code.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
                "code {code:?} is not SCREAMING_SNAKE_CASE"
            );
        }
    }

    #[test]
    fn code_is_not_added_to_the_display_message() {
        let err = TestkitError::Misuse("boom".into());
        assert!(!err.to_string().contains(err.code()));
    }
}
