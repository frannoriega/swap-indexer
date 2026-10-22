pub mod operation;

use solana_sdk::pubkey::Pubkey;

use crate::operation::Operation;

/**
 * Instruction parser trait, to be implemented depending on the DEX
 */
pub trait InstructionParser {
    fn parse(&self, instruction_data: &[u8], accounts: &[Pubkey]) -> Result<Option<Operation>, anyhow::Error>;
}
