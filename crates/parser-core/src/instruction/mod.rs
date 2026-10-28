use solana_sdk::pubkey::Pubkey;
use types::Instruction;

use crate::instruction::operation::Operation;

pub mod operation;

#[derive(Debug, Clone, Copy)]
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

pub trait Linkable {
    // Accounts this item reads value/state from
    fn consumes(&self) -> Vec<Pubkey>;

    // Accounts this item writes value/state to
    fn produces(&self) -> Vec<Pubkey>;

    // A grouping key for this item
    fn group(&self) -> u8;
}

impl Linkable for ParsedInstruction {
    fn consumes(&self) -> Vec<Pubkey> {
        self.operation.sources()
    }

    fn produces(&self) -> Vec<Pubkey> {
        self.operation.destinations()
    }

    fn group(&self) -> u8 {
        self.outer_idx
    }
}
