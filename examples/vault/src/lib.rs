#![no_std]

use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct Vault;

#[contractimpl]
impl Vault {
    pub fn ping(_env: Env) -> u32 {
        1
    }
}
