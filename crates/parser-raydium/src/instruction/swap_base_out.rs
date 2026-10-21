use solana_sdk::pubkey::Pubkey;
use types::unpack::little_endian::LittleEndianUnpacker;
use types::unpack::Unpacker;

use anyhow::anyhow;

use crate::instruction::{AmmInstruction, InstructionProcessor};

/**
 * Exact output swap instruction.
 * It models the exact amount to receive and the maximum amount to spend.
 */
#[derive(Debug, PartialEq)]
pub struct SwapBaseOutInstruction {
    pub source: Pubkey,
    pub destination: Pubkey,
    /**
     * The maximum amount of source tokens to be swapped.
     * This is a safety against a huge slippage.
     */
    pub max_amount_in: u64,
    /**
     * The amount of destination tokens requested.
     */
    pub amount_out: u64
}

pub struct SwapBaseOutInstructionProcessor;

impl InstructionProcessor for SwapBaseOutInstructionProcessor {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<super::AmmInstruction, anyhow::Error> {
        // Taken directly from the Raydium AMM repo
        // Commit: d26944b
        const ACCOUNT_LEN: usize = 17;
        if accounts.len() != ACCOUNT_LEN && accounts.len() != ACCOUNT_LEN + 1 {
            return Err(anyhow!("accounts count is not what we expected: {:}", accounts.len()));
        }
        const SOURCE_IDX: usize = 14;
        const DEST_IDX: usize = 15;

        let shift: usize = accounts.len() - ACCOUNT_LEN;

        let max_amount_in = unpacker.u64()?;
        let amount_out = unpacker.u64()?;
        Ok(AmmInstruction::SwapBaseOut(SwapBaseOutInstruction {
            source: accounts[SOURCE_IDX + shift],
            destination: accounts[DEST_IDX + shift],
            max_amount_in,
            amount_out,
        }))
    }
}

pub struct SwapBaseOutV2InstructionProcessor;

impl InstructionProcessor for SwapBaseOutV2InstructionProcessor {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<super::AmmInstruction, anyhow::Error> {
        // Taken directly from the Raydium AMM repo
        // Commit: d26944b
        const ACCOUNT_LEN: usize = 8;
        if accounts.len() != ACCOUNT_LEN {
            return Err(anyhow!("accounts count is not what we expected"));
        }
        const SOURCE_IDX: usize = 5;
        const DEST_IDX: usize = 6;

        let max_amount_in = unpacker.u64()?;
        let amount_out = unpacker.u64()?;
        Ok(AmmInstruction::SwapBaseOutV2(SwapBaseOutInstruction {
            source: accounts[SOURCE_IDX],
            destination: accounts[DEST_IDX],
            max_amount_in,
            amount_out,
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
        fn process_swap_base_out(max_amount_in: u64, amount_out: u64, accounts in addresses(17)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseOutInstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Ok(_)
            );
            let val = res.unwrap();
            assert_eq!(
                val,
                AmmInstruction::SwapBaseOut(SwapBaseOutInstruction { 
                    source: accounts[14], 
                    destination: accounts[15], 
                    max_amount_in, 
                    amount_out
                })
            );
        }

        #[test]
        fn process_swap_base_out_with_target_orders(max_amount_in: u64, amount_out: u64, accounts in addresses(18)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseOutInstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Ok(_)
            );
            let val = res.unwrap();
            assert_eq!(
                val,
                AmmInstruction::SwapBaseOut(SwapBaseOutInstruction { 
                    source: accounts[15], 
                    destination: accounts[16], 
                    max_amount_in, 
                    amount_out
                })
            );
        }

        #[test]
        fn process_swap_base_out_with_wrong_accounts(max_amount_in: u64, amount_out: u64, accounts in addresses(0..17)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseOutInstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Err(_)
            );
        }

        #[test]
        fn process_swap_base_out_v2(max_amount_in: u64, amount_out: u64, accounts in addresses(8)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseOutV2InstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Ok(_)
            );
            let val = res.unwrap();
            assert_eq!(
                val,
                AmmInstruction::SwapBaseOutV2(SwapBaseOutInstruction { 
                    source: accounts[5], 
                    destination: accounts[6], 
                    max_amount_in, 
                    amount_out
                })
            );
        }

        #[test]
        fn process_swap_base_out_v2_with_wrong_accounts(max_amount_in: u64, amount_out: u64, accounts in addresses(0..8)) {
            let mut instruction = Vec::<u8>::new();
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());
            let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
            let processor = SwapBaseOutV2InstructionProcessor;
            let res = processor.process(&mut unpacker, accounts.as_slice());

            assert_matches!(
                res,
                Err(_)
            );
        }
    }
}
