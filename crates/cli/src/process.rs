use std::{collections::HashMap, sync::LazyLock};

use anyhow::anyhow;
use parser_core::TxParser;
use parser_raydium::{RaydiumParser, instruction::AmmInstruction};
use solana_sdk::pubkey::{Pubkey};
use solana_sdk::pubkey;
use types::RawTx;

enum Parser {
    Raydium(TxParser<RaydiumParser, AmmInstruction>),
}

const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

static PROGRAMS: LazyLock<HashMap<Pubkey, Parser>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(
        RAYDIUM_AMM_V4,
        Parser::Raydium(TxParser::new(RaydiumParser)),
    );
    map
});

pub fn process(tx: RawTx) -> Result<(), anyhow::Error> {
    if let Some(err) = tx.status {
        return Err(anyhow!("Transaction failed: {:}", err));
    }
    Ok(())
}
