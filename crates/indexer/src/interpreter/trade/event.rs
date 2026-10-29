use solana_sdk::pubkey::Pubkey;

use crate::interpreter::Event;

pub struct TokenAmount {
    pub account: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub decimals: u8
}

pub struct TradeEvent {
    pub program_id: Pubkey,
    pub input: Pubkey,
    pub output: Pubkey,
    pub amount: u64,
    pub decimals: u8,
}

impl Event for TradeEvent {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn describe(&self) -> String {
        "Swap".to_owned()
    }
}
