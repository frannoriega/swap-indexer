pub mod unpack;

use solana_sdk::{
    clock::{Slot, UnixTimestamp},
    pubkey::Pubkey,
    signature::Signature,
    transaction::TransactionError,
};

/**
 * An abstraction on top of a client that fetches transaction from Solana's blockchain
 */
pub trait Fetcher {
    fn get_transaction(
        &self,
        signature: &Signature,
    ) -> impl Future<Output = Result<RawTx, anyhow::Error>>;
}

pub struct TokenBalance {
    pub account: Pubkey,
    pub mint: Pubkey,
    pub owner: Option<Pubkey>,
    pub amount: u64,
    pub decimals: u8,
}

/**
 * An already parsed Solana transaction
 */
#[derive(Default)]
pub struct RawTx {
    pub signature: Signature,
    pub slot: Slot,
    pub block_time: Option<UnixTimestamp>,
    pub account_keys: Vec<Pubkey>,
    pub instructions: Vec<Instruction>,
    pub status: Option<TransactionError>,

    pub fee: u64,
    pub pre_balances: Vec<u64>,
    pub post_balances: Vec<u64>,
    pub pre_token_balances: Vec<TokenBalance>,
    pub post_token_balances: Vec<TokenBalance>,
}

pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<Pubkey>,
    pub stack_depth: u32,
    pub outer_idx: u8,
    pub data: Vec<u8>,
}
