use solana_sdk::{clock::{Slot, UnixTimestamp}, pubkey::Pubkey, signature::Signature, transaction::TransactionError};

pub mod unpack;

/**
 * Instruction parser trait, to be implemented depending on the DEX
 */
pub trait InstructionParser<T> {
    fn parse(&self, instruction_data: &[u8]) -> Result<T, anyhow::Error>;
}

/**
 * An abstraction on top of a client that fetches transaction from Solana's blockchain
 */
pub trait Fetcher {
    fn get_transaction(&self, signature: &Signature) -> impl Future<Output = Result<RawTx, anyhow::Error>>;
}

/**
 * An already parsed Solana transaction
 */
pub struct RawTx {
    pub signature: Signature,
    pub slot: Slot,
    pub block_time: Option<UnixTimestamp>,
    pub account_keys: Vec<Pubkey>,
    pub instructions: Vec<Instruction>,
    pub status: Option<TransactionError>
}

pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<Pubkey>,
    pub stack_depth: u32,
    pub outer_idx: u8,
    pub data: Vec<u8>
}

pub struct ParsedTx<T> {
    pub signature: Signature,
    pub slot: Slot,
    pub block_time: Option<UnixTimestamp>,
    pub account_keys: Vec<Pubkey>,
    pub instructions: Vec<ParsedInstruction<T>>,
    pub status: Option<TransactionError>
}

pub struct ParsedInstruction<T> {
    pub program_id: Pubkey,
    pub accounts: Vec<Pubkey>,
    pub data: T
}
