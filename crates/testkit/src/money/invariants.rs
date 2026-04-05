use crate::core::{TestEnv, TestkitError};

/// A snapshot of value flows through a contract that streams or vests
/// money: what came in, what went out through each channel, and what is
/// left. Conservation means `deposited == withdrawn + refunded +
/// remaining`; any other outcome means the contract leaked or minted
/// value through rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conservation {
    /// Total value deposited into the contract.
    pub deposited: i128,
    /// Total value withdrawn by its intended recipient(s).
    pub withdrawn: i128,
    /// Total value refunded back to the depositor.
    pub refunded: i128,
    /// Value still held by the contract, not yet moved anywhere.
    pub remaining: i128,
}

impl Conservation {
    /// Assert that value is exactly conserved: `deposited == withdrawn +
    /// refunded + remaining`.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::AssertionFailed`] showing all four
    /// values and the delta between them if conservation does not hold.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::money::Conservation;
    ///
    /// let flows = Conservation {
    ///     deposited: 100,
    ///     withdrawn: 60,
    ///     refunded: 10,
    ///     remaining: 30,
    /// };
    /// flows.assert_holds();
    /// ```
    pub fn assert_holds(&self) {
        self.assert_within(0);
    }

    /// Assert that value is conserved within `tolerance`. Use only where a
    /// documented rounding tolerance is specified behavior, and state why
    /// at the call site.
    ///
    /// # Panics
    ///
    /// Panics with a [`TestkitError::AssertionFailed`] showing all four
    /// values and the delta if the discrepancy exceeds `tolerance`, or if
    /// summing the values would overflow `i128`.
    ///
    /// # Example
    ///
    /// ```
    /// use soroban_testkit::money::Conservation;
    ///
    /// // One stroop of rounding dust is documented as acceptable here.
    /// let flows = Conservation {
    ///     deposited: 100,
    ///     withdrawn: 60,
    ///     refunded: 10,
    ///     remaining: 29,
    /// };
    /// flows.assert_within(1);
    /// ```
    pub fn assert_within(&self, tolerance: i128) {
        let Some(accounted) = self
            .withdrawn
            .checked_add(self.refunded)
            .and_then(|v| v.checked_add(self.remaining))
        else {
            panic!(
                "{}",
                TestkitError::AssertionFailed(format!(
                    "conservation check overflowed summing withdrawn ({}) + refunded ({}) + remaining ({})",
                    self.withdrawn, self.refunded, self.remaining
                ))
            );
        };
        let Some(delta) = self.deposited.checked_sub(accounted) else {
            panic!(
                "{}",
                TestkitError::AssertionFailed(format!(
                    "conservation check overflowed computing deposited ({}) - accounted ({})",
                    self.deposited, accounted
                ))
            );
        };
        if delta.unsigned_abs() > tolerance.unsigned_abs() {
            panic!(
                "{}",
                TestkitError::AssertionFailed(format!(
                    "conservation violated: deposited={} withdrawn={} refunded={} remaining={} \
                     delta={delta} (tolerance={tolerance})",
                    self.deposited, self.withdrawn, self.refunded, self.remaining
                ))
            );
        }
    }
}

/// Drive a full lifecycle across `steps` points and assert conservation
/// holds at every one. `f` is called once per step with the environment
/// and the step index, and must return the conservation snapshot at that
/// point.
///
/// # Panics
///
/// Panics on the first step where [`Conservation::assert_holds`] fails.
///
/// # Example
///
/// ```
/// use soroban_testkit::core::TestEnv;
/// use soroban_testkit::money::{assert_conserved_over, Conservation};
///
/// let env = TestEnv::new();
/// assert_conserved_over(&env, 5, |_env, step| {
///     let withdrawn = step as i128 * 10;
///     Conservation {
///         deposited: 50,
///         withdrawn,
///         refunded: 0,
///         remaining: 50 - withdrawn,
///     }
/// });
/// ```
pub fn assert_conserved_over<F>(env: &TestEnv, steps: usize, f: F)
where
    F: Fn(&TestEnv, usize) -> Conservation,
{
    for step in 0..steps {
        f(env, step).assert_holds();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_holds_passes_when_conserved() {
        Conservation {
            deposited: 100,
            withdrawn: 60,
            refunded: 10,
            remaining: 30,
        }
        .assert_holds();
    }

    #[test]
    #[should_panic(
        expected = "conservation violated: deposited=100 withdrawn=60 refunded=10 remaining=25 delta=5"
    )]
    fn assert_holds_panics_with_full_diff_when_leaking() {
        Conservation {
            deposited: 100,
            withdrawn: 60,
            refunded: 10,
            remaining: 25,
        }
        .assert_holds();
    }

    #[test]
    fn assert_within_allows_documented_tolerance() {
        Conservation {
            deposited: 100,
            withdrawn: 60,
            refunded: 10,
            remaining: 29,
        }
        .assert_within(1);
    }

    #[test]
    #[should_panic(expected = "tolerance=1")]
    fn assert_within_still_rejects_beyond_tolerance() {
        Conservation {
            deposited: 100,
            withdrawn: 60,
            refunded: 10,
            remaining: 20,
        }
        .assert_within(1);
    }

    #[test]
    fn assert_conserved_over_runs_every_step() {
        use std::cell::Cell;

        let env = TestEnv::new();
        let calls = Cell::new(0);
        assert_conserved_over(&env, 4, |_env, step| {
            calls.set(calls.get() + 1);
            let withdrawn = step as i128 * 25;
            Conservation {
                deposited: 100,
                withdrawn,
                refunded: 0,
                remaining: 100 - withdrawn,
            }
        });
        assert_eq!(calls.get(), 4);
    }

    #[test]
    #[should_panic(expected = "conservation violated")]
    fn assert_conserved_over_panics_on_first_leaking_step() {
        let env = TestEnv::new();
        assert_conserved_over(&env, 3, |_env, step| Conservation {
            deposited: 100,
            withdrawn: step as i128,
            refunded: 0,
            remaining: 0,
        });
    }
}
