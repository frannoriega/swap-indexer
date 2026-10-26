use crate::instruction::operation::Operation;

pub mod operation;

#[derive(Debug)]
pub struct ParsedInstruction {
    pub operation: Operation,
    pub outer_idx: u8,
    pub stack_depth: u32,
}
