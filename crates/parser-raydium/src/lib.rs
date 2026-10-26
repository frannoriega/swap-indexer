pub mod instruction;
use std::{collections::HashMap, sync::LazyLock};

use anyhow::anyhow;
use parser_core::{InstructionParser, instruction::{ParsedInstruction, operation::Operation}};
use solana_sdk::pubkey::Pubkey;
use types::{Instruction, unpack::little_endian::LittleEndianUnpacker};

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
    map.insert(9, Processor::SwapBaseIn(SwapBaseInInstructionProcessor));
    map.insert(11, Processor::SwapBaseOut(SwapBaseOutInstructionProcessor));
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
    fn parse(&self, instruction: &Instruction) -> Result<Option<ParsedInstruction>, anyhow::Error> {
        let (id, rest) = instruction
            .data
            .split_first()
            .ok_or_else(|| anyhow!("invalid instruction"))?;
        let mut unpacker = LittleEndianUnpacker::new(rest);
        let processor = match PROCESSORS.get(id) {
            Some(p) => p,
            None => return Ok(None),
        };
        Ok(Some(ParsedInstruction {
            operation: processor.process(&mut unpacker, &instruction.accounts)?,
            outer_idx: instruction.outer_idx,
            stack_depth: instruction.stack_depth,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;
    use test_strategy::proptest;
    use types::proptest::{address, addresses};

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
        #[strategy(addresses(17..=18))] accounts: Vec<Pubkey>,
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
