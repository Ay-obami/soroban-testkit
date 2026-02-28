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
}
