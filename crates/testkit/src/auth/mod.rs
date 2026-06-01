//! Systematically prove that privileged entry points reject unauthorized
//! callers — the most common Soroban vulnerability class.

mod matrix;

pub use matrix::AuthMatrix;
