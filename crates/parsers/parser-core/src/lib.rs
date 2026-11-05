pub mod instruction;
pub mod system;

use solana_sdk::pubkey::Pubkey;
use types::unpack::{big_endian::BigEndianUnpacker, little_endian::LittleEndianUnpacker};

use crate::instruction::operation::Operation;

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
