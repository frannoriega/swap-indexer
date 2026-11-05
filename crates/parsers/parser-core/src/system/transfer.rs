use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use types::unpack::{Unpacker, little_endian::LittleEndianUnpacker};

use crate::{
    LEOperationParser,
    instruction::operation::{Operation, transfer::TransferOperation},
};

pub struct TransferOperationParser;

impl LEOperationParser for TransferOperationParser {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<Operation, anyhow::Error> {
        const ACCOUNTS_LEN: usize = 3;
        const SRC_IDX: usize = 0;
        const DST_IDX: usize = 1;
        const AUTH_IDX: usize = 2;
        if accounts.len() != ACCOUNTS_LEN && accounts.len() != ACCOUNTS_LEN + 1 {
            return Err(anyhow!(
                "accounts count is not what we expected: {:}",
                accounts.len()
            ));
        }
        let mint_shift = ACCOUNTS_LEN - accounts.len();
        let amount = unpacker.u64()?;
        let source = accounts[SRC_IDX];
        let mint = (mint_shift > 0).then(|| accounts[SRC_IDX + 1]);
        let dest = accounts[DST_IDX + mint_shift];
        let authority = accounts[AUTH_IDX + mint_shift];
        Ok(Operation::Transfer(TransferOperation {
            source,
            dest,
            mint,
            amount,
            authority,
        }))
    }
}
