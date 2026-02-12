# Security Policy

`soroban-testkit` is a testing library and CLI; it is not itself deployed
on-chain. That said, its `audit` subcommand and `AuthMatrix` helper make
security-relevant claims about contracts, so incorrect results matter.

## Reporting a vulnerability

Please report security issues privately via GitHub's
["Report a vulnerability"](../../security/advisories/new) feature on this
repository rather than opening a public issue. Include:

- The affected module or CLI command
- A minimal reproduction
- The impact you believe it has (e.g. `audit` missing a real bug,
  `AuthMatrix` reporting a false pass)

We aim to acknowledge reports within 5 business days.

## Scope notes

- `soroban-testkit audit` is a linter with a small set of heuristics, not a
  security product. A missed finding is a bug we want to fix, but the
  absence of an audit finding is never a security guarantee.
- The crate has zero non-dev network dependencies; any change that
  introduces network access at test time is treated as a regression.
