pub mod call_tree;
pub mod deltas;
pub mod execution;
pub mod interpreter;

use std::{
    collections::HashMap,
    sync::LazyLock,
};

use anyhow::anyhow;
use parser_core::{InstructionParser, instruction::ParsedInstruction, system::SplTokenParser};
use parser_raydium::RaydiumParser;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use types::RawTx;

use crate::deltas::compute_deltas;

enum Parser {
    Spl(SplTokenParser),
    Raydium(RaydiumParser),
}

impl InstructionParser for Parser {
    fn parse(
        &self,
        instruction: &types::Instruction,
    ) -> Result<Option<ParsedInstruction>, anyhow::Error> {
        match self {
            Parser::Spl(p) => p.parse(instruction),
            Parser::Raydium(p) => p.parse(instruction),
        }
    }
}

const SPL_TOKEN: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const SPL_TOKEN_2022: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

static PROGRAMS: LazyLock<HashMap<Pubkey, Parser>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(RAYDIUM_AMM_V4, Parser::Raydium(RaydiumParser));
    map.insert(SPL_TOKEN, Parser::Spl(SplTokenParser));
    map.insert(SPL_TOKEN_2022, Parser::Spl(SplTokenParser));
    map
});

pub fn process(tx: RawTx) -> Result<(), anyhow::Error> {
    if let Some(err) = tx.status {
        return Err(anyhow!("Transaction failed: {:}", err));
    }
    let _deltas = compute_deltas(&tx);
    for instr in &tx.instructions {
        let parsed_instr = match PROGRAMS.get(&instr.program_id) {
            Some(parser) => parser.parse(instr)?,
            None => continue,
        };
        let _operation = match parsed_instr {
            Some(op) => op,
            None => continue,
        };
    }
    Ok(())
}
