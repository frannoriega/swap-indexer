pub mod instruction;
pub mod system;

use solana_sdk::pubkey::Pubkey;
use types::{
    Instruction,
    unpack::{big_endian::BigEndianUnpacker, little_endian::LittleEndianUnpacker},
};

use crate::instruction::{ParsedInstruction, operation::Operation};

/**
 * Instruction parser trait, to be implemented depending on the DEX
 */
pub trait InstructionParser {
    fn parse(&self, instruction: &Instruction) -> Result<Option<ParsedInstruction>, anyhow::Error>;
}

pub trait LEOperationParser {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<Operation, anyhow::Error>;
}

pub trait BEOperationParser {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut BigEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<Operation, anyhow::Error>;
}
