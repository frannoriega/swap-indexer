use anyhow::anyhow;
use parser_core::LEOperationParser;
use trade_model::{Operation, swap::SwapInOperation};
use solana_sdk::pubkey::Pubkey;
use types::unpack::Unpacker;
use types::unpack::little_endian::LittleEndianUnpacker;

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

pub struct SwapBaseInOperationParser;

impl LEOperationParser<Operation> for SwapBaseInOperationParser {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<Operation, anyhow::Error> {
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
        let min_amount_out = unpacker.u64()?;
        Ok(Operation::SwapIn(SwapInOperation {
            amount_in,
            min_amount_out,
            source: accounts[SOURCE_IDX + shift],
            dest: accounts[DEST_IDX + shift],
        }))
    }
}

pub struct SwapBaseInV2OperationParser;

impl LEOperationParser<Operation> for SwapBaseInV2OperationParser {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<Operation, anyhow::Error> {
        // Taken directly from the Raydium AMM repo
        // Commit: d26944b
        const ACCOUNT_LEN: usize = 8;
        if accounts.len() != ACCOUNT_LEN {
            return Err(anyhow!("accounts count is not what we expected"));
        }
        const SOURCE_IDX: usize = 5;
        const DEST_IDX: usize = 6;

        let amount_in = unpacker.u64()?;
        let min_amount_out = unpacker.u64()?;
        Ok(Operation::SwapIn(SwapInOperation {
            amount_in,
            min_amount_out,
            source: accounts[SOURCE_IDX],
            dest: accounts[DEST_IDX],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;
    use test_strategy::proptest;
    use utils::proptest::addresses;

    #[proptest]
    fn process_swap_base_in(
        #[strategy(addresses(17))] accounts: Vec<Pubkey>,
        amount_in: u64,
        min_amount_out: u64,
    ) {
        let mut instruction = Vec::<u8>::new();
        instruction.extend_from_slice(&amount_in.to_le_bytes());
        instruction.extend_from_slice(&min_amount_out.to_le_bytes());
        let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
        let processor = SwapBaseInOperationParser;
        let res = processor.process(&mut unpacker, accounts.as_slice());

        assert_matches!(res, Ok(_));
        let val = res.unwrap();
        assert_eq!(
            val,
            Operation::SwapIn(SwapInOperation {
                source: accounts[14],
                dest: accounts[15],
                amount_in,
                min_amount_out
            })
        );
    }

    #[proptest]
    fn process_swap_base_in_with_target_orders(
        #[strategy(addresses(18))] accounts: Vec<Pubkey>,
        amount_in: u64,
        min_amount_out: u64,
    ) {
        let mut instruction = Vec::<u8>::new();
        instruction.extend_from_slice(&amount_in.to_le_bytes());
        instruction.extend_from_slice(&min_amount_out.to_le_bytes());
        let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
        let processor = SwapBaseInOperationParser;
        let res = processor.process(&mut unpacker, accounts.as_slice());

        assert_matches!(res, Ok(_));
        let val = res.unwrap();
        assert_eq!(
            val,
            Operation::SwapIn(SwapInOperation {
                source: accounts[15],
                dest: accounts[16],
                amount_in,
                min_amount_out
            })
        );
    }

    #[proptest]
    fn process_swap_base_in_with_wrong_accounts(
        #[strategy(addresses(0..17))] accounts: Vec<Pubkey>,
        amount_in: u64,
        min_amount_out: u64,
    ) {
        let mut instruction = Vec::<u8>::new();
        instruction.extend_from_slice(&amount_in.to_le_bytes());
        instruction.extend_from_slice(&min_amount_out.to_le_bytes());
        let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
        let processor = SwapBaseInOperationParser;
        let res = processor.process(&mut unpacker, accounts.as_slice());

        assert_matches!(res, Err(_));
    }

    #[proptest]
    fn process_swap_base_in_v2(
        #[strategy(addresses(8))] accounts: Vec<Pubkey>,
        amount_in: u64,
        min_amount_out: u64,
    ) {
        let mut instruction = Vec::<u8>::new();
        instruction.extend_from_slice(&amount_in.to_le_bytes());
        instruction.extend_from_slice(&min_amount_out.to_le_bytes());
        let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
        let processor = SwapBaseInV2OperationParser;
        let res = processor.process(&mut unpacker, accounts.as_slice());

        assert_matches!(res, Ok(_));
        let val = res.unwrap();
        assert_eq!(
            val,
            Operation::SwapIn(SwapInOperation {
                source: accounts[5],
                dest: accounts[6],
                amount_in,
                min_amount_out
            })
        );
    }

    #[proptest]
    fn process_swap_base_in_v2_with_wrong_accounts(
        #[strategy(addresses(0..8))] accounts: Vec<Pubkey>,
        amount_in: u64,
        min_amount_out: u64,
    ) {
        let mut instruction = Vec::<u8>::new();
        instruction.extend_from_slice(&amount_in.to_le_bytes());
        instruction.extend_from_slice(&min_amount_out.to_le_bytes());
        let mut unpacker = LittleEndianUnpacker::new(instruction.as_slice());
        let processor = SwapBaseInV2OperationParser;
        let res = processor.process(&mut unpacker, accounts.as_slice());

        assert_matches!(res, Err(_));
    }
}
