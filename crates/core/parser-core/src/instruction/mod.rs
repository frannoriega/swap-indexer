use solana_sdk::pubkey::Pubkey;
use types::Instruction;

#[derive(Debug, Clone, Copy)]
pub struct Parsed<T> {
    pub program_id: Pubkey,
    pub operation: T,
    pub outer_idx: u8,
    pub stack_depth: u32,
}

pub trait Resource {
    fn consumes(&self) -> Vec<Pubkey>;

    fn produces(&self) -> Vec<Pubkey>;
}

pub trait Linkable {
    // Accounts this item reads value/state from
    fn consumes(&self) -> Vec<Pubkey>;

    // Accounts this item writes value/state to
    fn produces(&self) -> Vec<Pubkey>;

    // A grouping key for this item
    fn group(&self) -> u8;
}

impl<T: Resource> Linkable for Parsed<T> {
    fn consumes(&self) -> Vec<Pubkey> {
        self.operation.consumes()
    }
    fn produces(&self) -> Vec<Pubkey> {
        self.operation.produces()
    }
    fn group(&self) -> u8 {
        self.outer_idx
    }
}

/**
 * Instruction parser trait, to be implemented depending on the DEX
 */
pub trait InstructionParser<T> {
    fn parse(&self, instruction: &Instruction) -> Result<Option<T>, anyhow::Error>;
}
