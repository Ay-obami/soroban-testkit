#!/usr/bin/env bash
#
# Run one CI test command with all network access removed.
#
# Usage:
#   test-no-network.sh all-targets  workspace unit + integration tests
#   test-no-network.sh doc          doc tests (README examples)
#   test-no-network.sh full         the whole `cargo test --workspace` run
#   test-no-network.sh coverage     cargo-llvm-cov coverage run
#
# This enforces the documented no-network test requirement
# (BUILD_SPEC.md §3, CONTRIBUTING.md scope): a test that needs the
# network must fail here, in CI, instead of passing on a connected
# machine. Dependencies are fetched outside the namespace; everything
# after `unshare` has no route to crates.io or anywhere else — the
# loopback device even stays down, because a fresh network namespace
# starts with no interfaces configured.

set -euo pipefail

case "${1:-}" in
  all-targets)
    set -- cargo test --workspace --all-targets --locked
    ;;
  doc)
    set -- cargo test --workspace --doc --locked
    ;;
  full)
    set -- cargo test --workspace --locked
    ;;
  coverage)
    set -- cargo llvm-cov --workspace --all-targets --lcov --output-path lcov.info
    ;;
  *)
    echo "usage: $0 all-targets|doc|full|coverage" >&2
    exit 2
    ;;
esac

# Everything the build below needs must already be on disk — the
# namespace we are about to enter cannot reach the network to fetch it.
cargo fetch --locked

# Enter the network namespace as root (sudo is passwordless on hosted
# runners), then drop back to the invoking user so files created under
# target/ keep the runner's ownership for the cache and cleanup steps.
exec sudo env "PATH=${PATH}" "HOME=${HOME}" unshare --net \
  --setuid "$(id -u)" \
  --setgid "$(id -g)" \
  -- "$@"
