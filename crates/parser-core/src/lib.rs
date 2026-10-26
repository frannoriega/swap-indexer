pub mod instruction;

use types::Instruction;

use crate::instruction::ParsedInstruction;

/**
 * Instruction parser trait, to be implemented depending on the DEX
 */
pub trait InstructionParser {
    fn parse(&self, instruction: &Instruction) -> Result<Option<ParsedInstruction>, anyhow::Error>;
}
