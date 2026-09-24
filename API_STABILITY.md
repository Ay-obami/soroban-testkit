# API stability and semver policy

Both workspace crates — `soroban-testkit` (the library) and
`soroban-testkit-cli` (the binary, installed as `soroban-testkit`) — share
one version number (the workspace root [`version`](Cargo.toml)) and follow
the rules in this document. This is the contract a downstream project can
rely on: what a version number means, what your code and tests may depend
on, and what is explicitly not covered.

## Reading a version

`soroban-testkit` is currently `0.x`. Cargo's SemVer rules apply, which has
one practical consequence for pre-1.0 crates:

| Example bump | Kind | May contain |
|---|---|---|
| `0.1.2` → `0.1.3` | PATCH | bug fixes that restore documented behavior; no new public API, no breaking changes |
| `0.1.3` → `0.2.0` | MINOR | new features **and** breaking changes |
| `0.2.0` → `1.0.0` | MAJOR | the same class of changes as a pre-1.0 minor, plus anything else held back until 1.0 |

Until 1.0, assume **any minor bump may break your code.** A `0.x` minor
release is the version where incompatible API changes land; a major bump is
only reached when the maintainers judge the API stable enough to commit to
backward-compatible minors for a while.

## What counts as a breaking change

A change is breaking if it can make a downstream crate fail to compile or a
downstream test start failing:

- **Removing, renaming, or re-signing any public item** — types, functions,
  methods, trait impls — in the library crate.
- **Changing the meaning of an existing public API** so that existing
  callers observe different behavior without opting in.
- **Changing a documented failure message.** Failure messages are a
  first-class, tested feature of this crate (`#[should_panic(expected = "...")]`
  tests pin them). Prefer appending detail to replacing an existing sentence;
  if the message must change, it is user-visible behavior and belongs in the
  changelog under the breaking-change heading.
- **Raising the pinned `soroban-sdk` major version** to one that breaks the
  contract builds of downstream users. This is why `COMPATIBILITY.md` exists:
  a testkit release and a `soroban-sdk` version travel together.
- **For the CLI crate only:** changing subcommand or flag names, removing a
  flag, changing a flag's accepted values, or changing documented exit
  behavior. The exact byte-for-byte stdout/stderr formatting is **not**
  stable API.

Adding a new public item (function, method, module) is never a breaking
change; it can ship in any bump that also honors the rules above.

## What is not covered by this policy

- **Seeded RNG output.** `TestEnv::with_seed(42)` produces *deterministic*
  address sequences within a release, but the concrete addresses are not
  guaranteed identical across `soroban-sdk` bumps. Never pin the exact
  addresses a seed produces.
- **CLI output formatting.** Human-readable text may change wording in a
  patch release; machine-gated behavior is covered under "breaking change"
  above.
- **Behavior gated on upstream `soroban-sdk` testutils** that changes when
  the SDK changes. If upstream stops exposing something this crate relies
  on, that is an upstream-driven change recorded in the changelog, not a
  policy violation.

## How the policy is enforced

- **Changelog on every user-facing change.** Per
  [`CONTRIBUTING.md`](CONTRIBUTING.md), every PR that changes user-facing
  behavior adds a `CHANGELOG.md` entry, and breaking changes are labeled as
  such. The [`docs` workflow](.github/workflows/docs.yml) fails a PR that
  removes a required process document.
- **Release checklist.** The [`Release` workflow](.github/workflows/release.yml)
  refuses to publish unless `CHANGELOG.md` has a section for the version
  being tagged and `COMPATIBILITY.md` still documents the pinned
  `soroban-sdk`.
- **`cargo semver-checks`.** Once the first release is published, the
  release workflow runs `cargo semver-checks` against the previous released
  version before publishing, so an unlabeled breaking change fails the
  release automatically.