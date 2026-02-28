//! The entry point every other module builds on: [`TestEnv`], a wrapper
//! around [`soroban_sdk::Env`], and [`TestkitError`], the error type
//! assertion helpers panic with.

mod env;
mod error;

pub use env::TestEnv;
pub use error::TestkitError;
