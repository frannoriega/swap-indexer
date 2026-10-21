use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use types::unpack::Unpacker;
use types::unpack::little_endian::LittleEndianUnpacker;

use crate::instruction::{AmmInstruction, InstructionProcessor};

/**
 * Exact input swap instruction.
 * It models the exact amount to spend and the least amount to receive.
 */
#[derive(Debug, PartialEq)]
pub struct SwapBaseInInstruction {
    pub source: Pubkey,
    pub destination: Pubkey,
    /**
     * The amount of source tokens a wallet is swapping for the destination token.
     */
    pub amount_in: u64,
    /**
     * The minimum amount of destination tokens (meaning, after swap) to be outputted.
     * This is meant as a safety guard in case of a huge slippage.
     */
    pub minimum_amount_out: u64,
}

pub struct SwapBaseInInstructionProcessor;

impl InstructionProcessor for SwapBaseInInstructionProcessor {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<AmmInstruction, anyhow::Error> {
        // Taken directly from the Raydium AMM repo
        // Commit: d26944b
        const ACCOUNT_LEN: usize = 17;
        if accounts.len() != ACCOUNT_LEN && accounts.len() != ACCOUNT_LEN + 1 {
            return Err(anyhow!(
                "accounts count is not what we expected: {:}",
                accounts.len()
            ));
        }
        const SOURCE_IDX: usize = 14;
        const DEST_IDX: usize = 15;

        let shift: usize = accounts.len() - ACCOUNT_LEN;

        let amount_in = unpacker.u64()?;
        let minimum_amount_out = unpacker.u64()?;
        Ok(AmmInstruction::SwapBaseIn(SwapBaseInInstruction {
            amount_in,
            minimum_amount_out,
            source: accounts[SOURCE_IDX + shift],
            destination: accounts[DEST_IDX + shift],
        }))
    }
}

pub struct SwapBaseInV2InstructionProcessor;

impl InstructionProcessor for SwapBaseInV2InstructionProcessor {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<AmmInstruction, anyhow::Error> {
        // Taken directly from the Raydium AMM repo
        // Commit: d26944b
        const ACCOUNT_LEN: usize = 8;
        if accounts.len() != ACCOUNT_LEN {
            return Err(anyhow!("accounts count is not what we expected"));
        }
        const SOURCE_IDX: usize = 5;
        const DEST_IDX: usize = 6;

        let amount_in = unpacker.u64()?;
        let minimum_amount_out = unpacker.u64()?;
        Ok(AmmInstruction::SwapBaseInV2(SwapBaseInInstruction {
            amount_in,
            minimum_amount_out,
            source: accounts[SOURCE_IDX],
            destination: accounts[DEST_IDX],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::assert_matches;
    use types::proptest::addresses;

    proptest! {

        #[test]
        fn process_swap_base_in(amount_in: u64, min_amount_out: u64, accounts in addresses(17)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&min_amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseInInstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Ok(_)
            );
            let val = res.unwrap();
            assert_eq!(
                val,
                AmmInstruction::SwapBaseIn(SwapBaseInInstruction {
                    source: accounts[14],
                    destination: accounts[15],
                    amount_in,
                    minimum_amount_out: min_amount_out
                })
            );
        }

        #[test]
        fn process_swap_base_in_with_target_orders(amount_in: u64, min_amount_out: u64, accounts in addresses(18)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&min_amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseInInstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Ok(_)
            );
            let val = res.unwrap();
            assert_eq!(
                val,
                AmmInstruction::SwapBaseIn(SwapBaseInInstruction {
                    source: accounts[15],
                    destination: accounts[16],
                    amount_in,
                    minimum_amount_out: min_amount_out
                })
            );
        }

        #[test]
        fn process_swap_base_in_with_wrong_accounts(amount_in: u64, min_amount_out: u64, accounts in addresses(0..17)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&min_amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseInInstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Err(_)
            );
        }

        #[test]
        fn process_swap_base_in_v2(amount_in: u64, min_amount_out: u64, accounts in addresses(8)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&min_amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseInV2InstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Ok(_)
            );
            let val = res.unwrap();
            assert_eq!(
                val,
                AmmInstruction::SwapBaseInV2(SwapBaseInInstruction {
                    source: accounts[5],
                    destination: accounts[6],
                    amount_in,
                    minimum_amount_out: min_amount_out
                })
            );
        }

        #[test]
        fn process_swap_base_in_v2_with_wrong_accounts(amount_in: u64, min_amount_out: u64, accounts in addresses(0..8)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&min_amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseInV2InstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Err(_)
            );
        }
    }
}
