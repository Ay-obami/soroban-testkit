# SDK migration guide

One entry per `soroban-sdk` version bump. Newest first.

Each entry follows the same structure: the SDK version change, what broke
or changed in this crate, and what downstream users need to update in their
own test code. If a change has no user-visible effect, say so explicitly —
that is itself useful information.

See [`CONTRIBUTING.md`](../CONTRIBUTING.md#sdk-upgrades-and-migration-guides)
for the policy on when an entry is required and what it must contain.

---

## Template

Copy this block when writing a new entry.

```markdown
## soroban-sdk X.Y.Z → A.B.C

**Merged:** <!-- PR number -->
**Caught by scheduled-sdk-check:** yes / no

### What changed in the SDK

<!-- One sentence per breaking or notable change. -->

### Impact on soroban-testkit

<!-- Which modules and public items were affected. "No impact" is a valid
     answer and should be stated explicitly. -->

### What downstream users need to change

<!-- What a test file using `soroban_testkit::prelude::*` needs to update.
     "No changes required" is a valid answer. -->
```

---

## soroban-sdk 27.0.6 (initial pinned version)

**Merged:** initial workspace commit  
**Caught by scheduled-sdk-check:** n/a — this is the baseline

### What changed in the SDK

This is the first pinned version; there is no prior version to diff
against for this crate.

### Impact on soroban-testkit

All modules (`core`, `ledger`, `money`, `events`, `tokens`, `auth`,
`ttl`, `prelude`) are built and tested against this version.

Key SDK surface this crate relies on, verified at 27.0.6:

| SDK feature | Used by | Notes |
|---|---|---|
| `soroban_sdk::testutils::Ledger` | `ledger` module | `set_sequence_number`, `timestamp`, `get` |
| `soroban_sdk::testutils::Address` | `core` | `Address::generate` |
| `register_stellar_asset_contract_v2` | `tokens` | No `decimals` parameter; always returns 7 |
| `soroban_sdk::testutils::Events` | `events` | `env.events().all()` |
| `soroban_sdk::testutils::{MockAuth, MockAuthInvoke}` | `auth` | Used in `AuthMatrix` |
| `env.storage().{instance,persistent,temporary}()` | `ttl` | TTL inspection via ledger |
| `env.events().publish(...)` | examples | Deprecated in 27.x; `#[contractevent]` is preferred |

### What downstream users need to change

No changes required (initial version).

### Notes for the next upgrade

Items to re-check when the SDK version is bumped:

- **`LEDGER_CLOSE_TIME_SECS`** in `crates/testkit/src/ledger/clock.rs`:
  this constant (currently `5`) is not exposed by the SDK and must be
  manually re-verified against observed network close times.
- **SAC decimals**: `register_stellar_asset_contract_v2` has no
  `decimals` parameter at 27.0.6.  If a future version adds one, the
  `tokens` module's `TestToken::decimals()` must be updated.
- **`env.events().publish(...)` deprecation**: the SDK deprecated this
  in 27.x in favour of `#[contractevent]`.  The testkit's event capture
  API (`EventLog`, `CapturedEvent`) currently works with both.  If the
  deprecated API is removed in a future version, all examples and the
  `events` test fixtures must migrate to `#[contractevent]`.
- **TTL / state-archival parameters**: the `ttl` module reads these from
  the live ledger info rather than hardcoding them; re-verify that the
  SDK still exposes the required ledger info fields.
- **Scheduled-sdk-check**: the weekly CI job (`scheduled-sdk-check` in
  `.github/workflows/ci.yml`) runs `cargo update -p soroban-sdk` and
  builds the full workspace.  If it opens an auto-issue, that is the
  trigger for this migration process.
