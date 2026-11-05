pub mod instruction;
use std::{collections::HashMap, sync::LazyLock};

use anyhow::anyhow;
use parser_core::instruction::InstructionParser;
use parser_core::LEOperationParser;
use solana_sdk::pubkey::Pubkey;
use trade_model::Operation;
use types::{Instruction, unpack::little_endian::LittleEndianUnpacker};

use crate::instruction::{
    swap_base_in::{SwapBaseInOperationParser, SwapBaseInV2OperationParser},
    swap_base_out::{SwapBaseOutOperationParser, SwapBaseOutV2OperationParser},
};

pub struct RaydiumParser;

enum Parser {
    SwapBaseIn(SwapBaseInOperationParser),
    SwapBaseOut(SwapBaseOutOperationParser),
    SwapBaseInV2(SwapBaseInV2OperationParser),
    SwapBaseOutV2(SwapBaseOutV2OperationParser),
}

impl LEOperationParser<Operation> for Parser {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<Operation, anyhow::Error> {
        match self {
            Parser::SwapBaseIn(proc) => proc.process(unpacker, accounts),
            Parser::SwapBaseOut(proc) => proc.process(unpacker, accounts),
            Parser::SwapBaseInV2(proc) => proc.process(unpacker, accounts),
            Parser::SwapBaseOutV2(proc) => proc.process(unpacker, accounts),
        }
    }
}

static PARSERS: LazyLock<HashMap<u8, Parser>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(9, Parser::SwapBaseIn(SwapBaseInOperationParser));
    map.insert(11, Parser::SwapBaseOut(SwapBaseOutOperationParser));
    map.insert(16, Parser::SwapBaseInV2(SwapBaseInV2OperationParser));
    map.insert(17, Parser::SwapBaseOutV2(SwapBaseOutV2OperationParser));

    map
});

impl InstructionParser<Operation> for RaydiumParser {
    fn parse(&self, instruction: &Instruction) -> Result<Option<Operation>, anyhow::Error> {
        let (id, rest) = instruction
            .data
            .split_first()
            .ok_or_else(|| anyhow!("invalid instruction"))?;
        let mut unpacker = LittleEndianUnpacker::new(rest);
        let parser = match PARSERS.get(id) {
            Some(p) => p,
            None => return Ok(None),
        };
        Ok(Some(parser.process(&mut unpacker, &instruction.accounts)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;
    use test_strategy::proptest;
    use utils::proptest::{address, addresses};

    #[proptest]
    fn parses_swap_base_in(
        #[strategy(address())] program_id: Pubkey,
        #[strategy(addresses(17..=18))] accounts: Vec<Pubkey>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) {
        let parser = RaydiumParser;
        let mut data = Vec::<u8>::new();
        data.push(9u8);
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());
        let instruction = Instruction {
            program_id,
            data,
            accounts,
            stack_depth: 0,
            outer_idx: 0,
        };

        assert_matches!(parser.parse(&instruction), Ok(_));
    }

    #[proptest]
    fn parses_swap_base_out(
        #[strategy(address())] program_id: Pubkey,
        #[strategy(addresses(17..=18))] accounts: Vec<Pubkey>,
        max_amount_in: u64,
        amount_out: u64,
    ) {
        let parser = RaydiumParser;
        let mut data = Vec::<u8>::new();
        data.push(11u8);
        data.extend_from_slice(&max_amount_in.to_le_bytes());
        data.extend_from_slice(&amount_out.to_le_bytes());
        let instruction = Instruction {
            program_id,
            accounts,
            data,
            outer_idx: 0,
            stack_depth: 0,
        };

        assert_matches!(parser.parse(&instruction), Ok(_));
    }

    #[proptest]
    fn parses_swap_base_in_v2(
        #[strategy(address())] program_id: Pubkey,
        #[strategy(addresses(8))] accounts: Vec<Pubkey>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) {
        let parser = RaydiumParser;
        let mut data = Vec::<u8>::new();
        data.push(16u8);
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());
        let instruction = Instruction {
            program_id,
            accounts,
            data,
            outer_idx: 0,
            stack_depth: 0,
        };

        assert_matches!(parser.parse(&instruction), Ok(_));
    }

    #[proptest]
    fn parses_swap_base_out_v2(
        #[strategy(address())] program_id: Pubkey,
        #[strategy(addresses(8))] accounts: Vec<Pubkey>,
        max_amount_in: u64,
        amount_out: u64,
    ) {
        let parser = RaydiumParser;
        let mut data = Vec::<u8>::new();
        data.push(17u8);
        data.extend_from_slice(&max_amount_in.to_le_bytes());
        data.extend_from_slice(&amount_out.to_le_bytes());
        let instruction = Instruction {
            program_id,
            accounts,
            data,
            outer_idx: 0,
            stack_depth: 0,
        };

        assert_matches!(parser.parse(&instruction), Ok(_));
    }
}
