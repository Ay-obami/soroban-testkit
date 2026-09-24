//! The entry point every other module builds on: [`TestEnv`], a wrapper
//! around [`soroban_sdk::Env`], and [`TestkitError`], the error type
//! assertion helpers panic with.

mod env;
mod error;

pub use env::{Actor, AddressIter, TestEnv, MAX_ADDRESS_BATCH_SIZE};
pub use error::TestkitError;
