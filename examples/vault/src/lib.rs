#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contracttype]
pub enum DataKey {
    Owner,
    Balance,
}

/// Errors this contract returns from its `try_*` entry points.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    NotInitialized = 1,
    InsufficientBalance = 2,
}

#[contract]
pub struct Vault;

#[contractimpl]
impl Vault {
    pub fn initialize(env: Env, owner: Address) {
        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage().instance().set(&DataKey::Balance, &0i128);
    }

    pub fn owner(env: Env) -> Result<Address, VaultError> {
        env.storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(VaultError::NotInitialized)
    }

    pub fn balance(env: Env) -> Result<i128, VaultError> {
        env.storage()
            .instance()
            .get(&DataKey::Balance)
            .ok_or(VaultError::NotInitialized)
    }

    pub fn deposit(env: Env, amount: i128) -> Result<(), VaultError> {
        let balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Balance)
            .ok_or(VaultError::NotInitialized)?;
        env.storage()
            .instance()
            .set(&DataKey::Balance, &(balance + amount));
        Ok(())
    }

    /// Deliberately missing the owner auth check: any caller can drain the
    /// vault. Kept alongside `withdraw_checked` so soroban-testkit's own
    /// tests can demonstrate `AuthMatrix::assert_enforced` catching a real
    /// missing-auth vulnerability, and passing once it's fixed.
    pub fn withdraw(env: Env, amount: i128) -> Result<(), VaultError> {
        let balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Balance)
            .ok_or(VaultError::NotInitialized)?;
        if balance < amount {
            return Err(VaultError::InsufficientBalance);
        }
        env.storage()
            .instance()
            .set(&DataKey::Balance, &(balance - amount));
        Ok(())
    }

    /// The fixed version of `withdraw`: only the owner may withdraw.
    pub fn withdraw_checked(env: Env, amount: i128) -> Result<(), VaultError> {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .ok_or(VaultError::NotInitialized)?;
        owner.require_auth();
        Self::withdraw(env, amount)
    }
}
