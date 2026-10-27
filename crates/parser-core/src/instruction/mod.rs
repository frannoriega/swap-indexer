use solana_sdk::pubkey::Pubkey;
use types::Instruction;

use crate::instruction::operation::Operation;

pub mod operation;

#[derive(Debug)]
pub struct ParsedInstruction {
    pub program_id: Pubkey,
    pub operation: Operation,
    pub outer_idx: u8,
    pub stack_depth: u32,
}

impl ParsedInstruction {
    pub fn unknown(instruction: &Instruction) -> ParsedInstruction {
        ParsedInstruction {
            program_id: instruction.program_id,
            operation: Operation::Unknown,
            outer_idx: instruction.outer_idx,
            stack_depth: instruction.stack_depth,
        }
    }
}
