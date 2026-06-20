//! Ledger-entry TTL inspection and expiry simulation.
//!
//! Contracts that fail to bump TTL break in production through silent data
//! loss, and the failure mode is close to untestable by hand — this module
//! makes it a normal assertion.

mod expiry;

pub use expiry::StorageKind;
