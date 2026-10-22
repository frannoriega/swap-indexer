pub mod instruction;
use std::{collections::HashMap, sync::LazyLock};

use anyhow::anyhow;
use parser_core::{InstructionParser, operation::Operation};
use solana_sdk::pubkey::Pubkey;
use types::unpack::little_endian::LittleEndianUnpacker;

use crate::instruction::{
    InstructionProcessor,
    swap_base_in::{SwapBaseInInstructionProcessor, SwapBaseInV2InstructionProcessor},
    swap_base_out::{SwapBaseOutInstructionProcessor, SwapBaseOutV2InstructionProcessor},
};

pub struct RaydiumParser;

enum Processor {
    SwapBaseIn(SwapBaseInInstructionProcessor),
    SwapBaseOut(SwapBaseOutInstructionProcessor),
    SwapBaseInV2(SwapBaseInV2InstructionProcessor),
    SwapBaseOutV2(SwapBaseOutV2InstructionProcessor),
}

impl InstructionProcessor for Processor {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[Pubkey],
    ) -> Result<Operation, anyhow::Error> {
        match self {
            Processor::SwapBaseIn(proc) => proc.process(unpacker, accounts),
            Processor::SwapBaseOut(proc) => proc.process(unpacker, accounts),
            Processor::SwapBaseInV2(proc) => proc.process(unpacker, accounts),
            Processor::SwapBaseOutV2(proc) => proc.process(unpacker, accounts),
        }
    }
}

static PROCESSORS: LazyLock<HashMap<u8, Processor>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(
        9,
        Processor::SwapBaseIn(SwapBaseInInstructionProcessor),
    );
    map.insert(
        11,
        Processor::SwapBaseOut(SwapBaseOutInstructionProcessor),
    );
    map.insert(
        16,
        Processor::SwapBaseInV2(SwapBaseInV2InstructionProcessor),
    );
    map.insert(
        17,
        Processor::SwapBaseOutV2(SwapBaseOutV2InstructionProcessor),
    );

    map
});

impl InstructionParser for RaydiumParser {
    fn parse(
        &self,
        instruction_data: &[u8],
        accounts: &[Pubkey],
    ) -> Result<Option<Operation>, anyhow::Error> {
        let (id, rest) = instruction_data
            .split_first()
            .ok_or_else(|| anyhow!("invalid instruction"))?;
        let mut unpacker = LittleEndianUnpacker::new(rest);
        let processor = match PROCESSORS.get(id) {
            Some(p) => p,
            None => return Ok(None),
        };
        Ok(Some(processor.process(&mut unpacker, accounts)?))
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
        fn parses_swap_base_in(amount_in: u64, minimum_amount_out: u64, accounts in addresses(17..=18)) {
            let parser = RaydiumParser;
            let mut instruction = Vec::<u8>::new();
            instruction.push(9u8);
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&minimum_amount_out.to_le_bytes());

            assert_matches!(parser.parse(instruction.as_slice(), accounts.as_slice()), Ok(_));
        }

        #[test]
        fn parses_swap_base_out(max_amount_in: u64, amount_out: u64, accounts in addresses(17..=18)) {
            let parser = RaydiumParser;
            let mut instruction = Vec::<u8>::new();
            instruction.push(11u8);
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());

            assert_matches!(parser.parse(instruction.as_slice(), accounts.as_slice()), Ok(_));
        }

        #[test]
        fn parses_swap_base_in_v2(amount_in: u64, minimum_amount_out: u64, accounts in addresses(8)) {
            let parser = RaydiumParser;
            let mut instruction = Vec::<u8>::new();
            instruction.push(16u8);
            instruction.extend_from_slice(&amount_in.to_le_bytes());
            instruction.extend_from_slice(&minimum_amount_out.to_le_bytes());

            assert_matches!(parser.parse(instruction.as_slice(), accounts.as_slice()), Ok(_));
        }

        #[test]
        fn parses_swap_base_out_v2(max_amount_in: u64, amount_out: u64, accounts in addresses(8)) {
            let parser = RaydiumParser;
            let mut instruction = Vec::<u8>::new();
            instruction.push(17u8);
            instruction.extend_from_slice(&max_amount_in.to_le_bytes());
            instruction.extend_from_slice(&amount_out.to_le_bytes());

            assert_matches!(parser.parse(instruction.as_slice(), accounts.as_slice()), Ok(_));
        }
    }
}
