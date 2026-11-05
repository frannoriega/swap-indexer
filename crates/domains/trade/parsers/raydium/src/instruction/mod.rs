pub mod swap_base_in;
pub mod swap_base_out;

use crate::instruction::swap_base_in::SwapBaseInInstruction;
use crate::instruction::swap_base_out::SwapBaseOutInstruction;

#[derive(Debug, PartialEq)]
pub enum AmmInstruction {
    SwapBaseIn(SwapBaseInInstruction),
    SwapBaseOut(SwapBaseOutInstruction),
    SwapBaseInV2(SwapBaseInInstruction),
    SwapBaseOutV2(SwapBaseOutInstruction),
}
