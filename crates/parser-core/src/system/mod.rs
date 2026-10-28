pub mod transfer;
use std::{collections::HashMap, sync::LazyLock};

use anyhow::anyhow;
use types::{Instruction, unpack::little_endian::LittleEndianUnpacker};

use crate::{
    LEOperationParser,
    instruction::{InstructionParser, operation::Operation},
    system::transfer::TransferOperationParser,
};

enum Parser {
    Transfer(TransferOperationParser),
}

impl LEOperationParser for Parser {
    fn process<'a>(
        &'a self,
        unpacker: &'a mut LittleEndianUnpacker<'a>,
        accounts: &[solana_sdk::pubkey::Pubkey],
    ) -> Result<crate::instruction::operation::Operation, anyhow::Error> {
        match self {
            Parser::Transfer(p) => p.process(unpacker, accounts),
        }
    }
}

static PARSERS: LazyLock<HashMap<u8, Parser>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(3, Parser::Transfer(TransferOperationParser));
    map.insert(12, Parser::Transfer(TransferOperationParser));
    map
});

pub struct SplTokenParser;

impl InstructionParser<Operation> for SplTokenParser {
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
