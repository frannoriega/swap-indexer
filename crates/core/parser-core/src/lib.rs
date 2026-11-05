pub mod instruction;

use solana_sdk::pubkey::Pubkey;
use types::unpack::{big_endian::BigEndianUnpacker, little_endian::LittleEndianUnpacker};

/// Parses the body of a little-endian encoded instruction into some domain payload `T`.
/// This crate knows nothing about what `T` is; each domain defines its own.
pub trait LEOperationParser<T> {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<T, anyhow::Error>;
}

/// Big-endian counterpart of [LEOperationParser].
pub trait BEOperationParser<T> {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut BigEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<T, anyhow::Error>;
}
