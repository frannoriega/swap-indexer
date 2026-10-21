pub mod swap_base_in;
pub mod swap_base_out;

use solana_sdk::pubkey::Pubkey;
use types::unpack::little_endian::LittleEndianUnpacker;

use crate::instruction::swap_base_in::SwapBaseInInstruction;
use crate::instruction::swap_base_out::SwapBaseOutInstruction;

#[derive(Debug, PartialEq)]
pub enum AmmInstruction {
    SwapBaseIn(SwapBaseInInstruction),
    SwapBaseOut(SwapBaseOutInstruction),
    SwapBaseInV2(SwapBaseInInstruction),
    SwapBaseOutV2(SwapBaseOutInstruction),
}

pub trait InstructionProcessor {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<AmmInstruction, anyhow::Error>;
}
