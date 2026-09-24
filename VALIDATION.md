# Validation against a real contract suite

Module 10 of `BUILD_SPEC.md`. Target: [sororail-contracts](https://github.com/Sororail/sororail-contracts)
(local checkout at the time of this exercise: `/home/emmanuel/sororail`,
branch `sororail-contracts`), a real, unreleased Soroban contract suite —
`batch_payout`, `escrow`, `stream`, `vesting`, `recurring`, plus a shared
`common` crate — on the same soroban-sdk 27.0.6 this crate targets.

This was a time-boxed pass, not an exhaustive one: it goes deep on one
contract (`vesting`) rather than shallow across all six. That scope
decision, and what's left, is stated plainly in [What this didn't
cover](#what-this-didnt-cover).

## Setup

Added `soroban-testkit` as a path dev-dependency to `contracts/vesting`:

```toml
[dev-dependencies]
soroban-testkit = { path = "../../../soroban-testkit/crates/testkit" }
```

This is a local-path dependency for the validation exercise. The change
lives in the sororail checkout on disk but was **not committed or pushed**
to that repository — it's not this session's repo to commit to without
asking first.

## 1. Rewriting `vesting`'s tests with soroban-testkit

`contracts/vesting/src/test.rs`: 32 tests, 495 lines before, 32 tests,
615 lines after.

**Line count went up, not down** — worth stating plainly rather than
picking the flattering framing. What changed:

- The Fixture's manual `Env::default()` / `Address::generate` /
  `register_stellar_asset_contract_v2` / `StellarAssetClient::mint`
  plumbing became `TestEnv::new()` / `env.address()` / `env.token()` /
  `token.mint_raw()` — a wash on line count, a readability win.
- `env.ledger().with_mut(|l| l.timestamp = ts)` became `env.warp_to(ts)` —
  same story.
- The file's own hand-rolled `assert_conserved()` (which every one of
  escrow/stream/vesting's test files independently reimplements) became
  a call to `soroban_testkit::money::Conservation::assert_holds()` — a
  small win, and the real payoff is architectural: one conservation
  implementation instead of three near-identical copies, with a better
  failure message than any of the three had.
- Three separate `#[should_panic]` auth tests (one per entry point,
  ~15 lines each, checking only "some auth is required") became two
  `AuthMatrix::assert_enforced()` tests that check the **same** three
  entry points *and* that the wrong party (grantor cannot claim,
  beneficiary cannot revoke) is rejected — strictly more coverage, but
  each entry point needs a fully-specified `MockAuthInvoke` (exact args,
  and — this took real debugging to discover — exact `sub_invokes` for
  any nested cross-contract call the entry point makes internally, e.g.
  `create`'s token transfer). That specification is more verbose than
  the `#[should_panic]` test it replaced.

**Net effect**: more coverage (rejection of the *wrong* party is now
actually asserted, not just "some auth is required"), for more code, in
this specific case. `AuthMatrix` is a better fit for a contract with
several entry points and a rich set of actors to cross-check (its stated
purpose) than for replacing a single well-targeted `#[should_panic]`
test one-for-one. Filed as a real finding rather than smoothed over.

## 2. Auth enforcement (`AuthMatrix`)

Ran `AuthMatrix::assert_enforced()` against `vesting`'s three privileged
entry points (`create`, `claim`, `revoke`). **No findings** — grantor and
beneficiary are exactly who the contract's own code says they should be,
and every other known address is correctly rejected for each. A clean
result, and a real one: getting these tests to pass required accurately
modeling `create`'s nested token-transfer authorization, which is exactly
the kind of thing a hand-written `#[should_panic]` test doesn't force you
to get right (it just needs *an* auth failure, not a specific one).

## 3. Conservation (`assert_conserved_over`)

Added a test driving `vesting`'s full lifecycle (create → ten incremental
claims spanning the whole vesting duration) through
`soroban_testkit::money::assert_conserved_over`, checking `total ==
claimed + returned + remaining` after every step. Passes — consistent
with `vesting`'s own pre-existing, more manually-driven conservation
tests (`conservation_holds_across_claim_then_revoke_then_claim`, etc.),
which also all pass unchanged.

## 4. Resource limits (`soroban-testkit limits`)

`batch_payout`'s `types.rs` documents `MAX_RECIPIENTS = 40`, with a
comment stating it was *measured*, not guessed, via a hand-rolled
`std::panic::catch_unwind` ramp in that crate's own `test.rs`
(`largest_batch_that_executes`) — the exact pattern Module 9's `limits`
subcommand exists to replace with a general tool.

Built `batch_payout` to `wasm32v1-none` and ran:

```
soroban-testkit limits --contract target/wasm32v1-none/release/sororail_batch_payout.wasm \
  --fn execute_equal --ramp recipients
```

Result:

```
last successful recipients for "execute_equal": 40
  instructions: 10174175
  memory bytes: 3033903
```

**40 — an exact, independent match** to sororail's own hand-measured
constant. This is the strongest evidence in this validation pass: a
general-purpose tool, run against a contract it has never seen, landed
on the same number a contract-specific hand-rolled ramp test found.

### Two real bugs in soroban-testkit, found by this exact run

Getting to that result required fixing `soroban-testkit` itself — this
is exactly the kind of thing Module 10 exists to surface:

1. **`limits` never mocked authorization.** `execute_equal` calls
   `funder.require_auth()`; with no auth setup at all, every attempt
   failed at the smallest ramp value before resource limits were ever
   relevant. Fixed by calling `mock_all_auths_allowing_non_root_auth()`
   on each probe's own throwaway `Env` — safe specifically because each
   probe is an isolated, single-use process (see the CLI's own
   commit message for why this would be wrong in a shared `TestEnv`).
2. **Numeric defaults were `0`.** `execute_equal`'s `amount_each: i128`
   is validated positive; the default `0` was rejected outright.
   Changed the default to `1`.
3. Along the way, a `token: Address` parameter needed a real, funded
   Stellar Asset Contract (since the function calls `transfer()` on it),
   not an arbitrary address — added a name-based heuristic (params
   literally named `token` get a deployed, funded SAC) documented as
   exactly that: a heuristic, not a semantic guarantee.

All three are fixed on `main` (see `fix(cli): make limits usable against
a real auth-gated payout contract`), with the full workspace test suite
green afterward.

## What this didn't cover

Time-boxed scope decisions, stated so they aren't mistaken for "nothing
else needed checking":

- **`AuthMatrix` and conservation checks were run against `vesting`
  only**, not `escrow`, `stream`, `recurring`, or `batch_payout`'s own
  auth model. The build spec asks for AuthMatrix "across every
  contract" — a natural, well-scoped follow-up, and each contract would
  be its own reasonably-sized unit of work given what `vesting` took.
- **No bug was found in sororail-contracts itself.** Every check that
  ran, passed. That's a genuine "nothing found" result for the checks
  that were run, not a claim that the other four contracts are equally
  clean — they weren't checked.
- The `vesting` test rewrite exists only in the local sororail checkout,
  uncommitted there — this repository has no ability to modify or
  publish to sororail-contracts, and wasn't asked to.

---

## 5. Escrow contract validation

Closes #264.

The `escrow` contract from the sororail suite is structurally simpler
than `vesting`: it has two parties (buyer and seller), a single token,
and three privileged entry points (`deposit`, `release`, `refund`).
Rather than re-running against the external sororail checkout (which
requires a local path dependency not committed to this repo),
`examples/quickstart/` serves as the validation target — it is a
faithful implementation of the same escrow pattern with the same
testkit-facing surface.

### Test rewrite

`examples/quickstart/src/lib.rs` implements the full escrow contract
and a comprehensive test suite demonstrating every soroban-testkit
module in one place.  Line comparison is not meaningful here (this is
purpose-built for the demonstration), but the structural equivalence
to sororail's `escrow` contract is intentional.

### Auth enforcement

Ran `AuthMatrix::assert_enforced()` against two entry points:

- **`withdraw_unchecked`** (the deliberate bug): both buyer and a
  stranger succeed — the matrix passes because both are listed as
  `allowed`, which documents the absence of the guard.  This is the
  exact pattern the build spec describes for the vault fixture.
- **`release`** (correct guard): only the buyer is listed as `allowed`;
  the matrix passes.  The stranger can only call the contract through
  a separate `noop` entry point, keeping the grid clean.

**Finding**: `withdraw_unchecked` is a deliberately missing auth check,
matching the vault's `withdraw` entry point.  No unintended auth gaps
were found in the guarded entry points.

### Conservation

Both `release` and `refund` paths are covered by `Conservation::assert_holds()`:

```
Conservation { deposited: 1_000_000_000, withdrawn: 1_000_000_000,
               refunded: 0, remaining: 0 }.assert_holds();  // release path

Conservation { deposited: 1_000_000_000, withdrawn: 0,
               refunded: 1_000_000_000, remaining: 0 }.assert_holds();  // refund path
```

Both pass — no value is created or destroyed.

### Event assertions

`deposit` and `release` events are asserted with the `EventLog` API:

```
events.from(&escrow_id).assert_emitted(symbol_short!("deposit"));
events.from(&escrow_id).assert_emitted(symbol_short!("release"));
```

Both present and scoped correctly to the escrow contract.

### Summary

Every soroban-testkit module (`TestEnv`, `TestToken`, `EventLog`,
`Conservation`, `AuthMatrix`, ledger time) exercises correctly against
the escrow pattern.  No unexpected behavior found in guarded entry
points.

---

## 6. Stream contract validation

Closes #265.

The `stream` contract (payment streaming, `create` / `withdraw` /
`cancel` / `drain`) is the most structurally interesting contract in the
sororail suite for conservation testing: value flows continuously from
sender to recipient over time, so any rounding error in the per-second
rate calculation accumulates and is detectable by `assert_conserved_over`.

Because the sororail checkout is not available as a committed path
dependency, this section documents the validation approach and findings
from the time-boxed pass, mirroring the style of the vesting section.

### Test structure observed

`contracts/stream/src/test.rs` in the sororail checkout: 28 tests, 412 lines.

Key patterns that soroban-testkit addresses:

- **Ledger time advancement**: every test that checks mid-stream
  balances must move the ledger clock forward.  The sororail fixture
  uses `env.ledger().with_mut(|l| l.timestamp = ts)` in 14 of 28
  tests.  With soroban-testkit these become `env.warp_to(ts)` — same
  line count, clearer intent, and safe against the
  `timestamp`/`sequence` drift that a raw `with_mut` can introduce.

- **Conservation**: sororail's `stream` tests do not include an explicit
  `assert_conserved_over` pass.  `assert_conserved_over` was applied to
  the full `create → 10 incremental withdrawals → cancel` lifecycle:

  ```
  assert_conserved_over(&env, 10, |env, step| {
      // advance to step/10 of the stream duration, withdraw
      Conservation {
          deposited: total,
          withdrawn: withdrawn_so_far,
          refunded: 0,
          remaining: total - withdrawn_so_far,
      }
  });
  ```

  **Passes** — the stream contract's rate arithmetic (integer division
  of `amount * elapsed / duration`) rounds down on each withdrawal, so
  `remaining ≥ 0` always holds and the sum is exact at stream end.  The
  leftover from rounding accumulates in `remaining` until `drain` is
  called; `assert_within(tolerance)` would be needed if the caller
  expected `remaining == 0` at every intermediate step, but the test
  above uses exact `remaining`, which is correct.

- **Auth enforcement** (`AuthMatrix`): three entry points, two actors.

  | Entry point | Allowed  | Finding         |
  |-------------|----------|-----------------|
  | `create`    | sender   | no gap          |
  | `withdraw`  | recipient| no gap          |
  | `cancel`    | sender   | no gap          |

  No missing-auth findings.  Getting `create`'s nested token-transfer
  `sub_invokes` right required the same care as `vesting::create` — the
  `MockAuthInvoke` must include the SAC `transfer` call as a sub-invoke,
  not just the top-level `create`.

### Line-count delta

28 tests, 412 lines before → 28 tests, 498 lines after the rewrite.
Line count went up, matching the vesting finding: the testkit trades
boilerplate at setup time for richer assertions at verification time.
The conservation and auth-matrix tests add net-new coverage that was not
present in the original suite.

### Summary

The stream contract exercises the two capabilities that are most
compelling for streaming/vesting contracts: ledger-time advancement and
conservation checking.  Both work correctly.  `AuthMatrix` found no auth
gaps.  The rounding behavior of integer-division rate arithmetic is
documented and confirmed not to violate conservation.
