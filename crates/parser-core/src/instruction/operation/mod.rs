pub mod swap;

use solana_sdk::pubkey::Pubkey;

use crate::instruction::operation::swap::{SwapInOperation, SwapOutOperation};

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Operation {
    SwapIn(SwapInOperation),
    SwapOut(SwapOutOperation),
}

impl Operation {
    pub fn sources(&self) -> Vec<Pubkey> {
        match self {
            Operation::SwapIn(s) => vec![s.source],
            Operation::SwapOut(s) => vec![s.source],
        }
    }

    pub fn destinations(&self) -> Vec<Pubkey> {
        match self {
            Operation::SwapIn(s) => vec![s.dest],
            Operation::SwapOut(s) => vec![s.dest],
        }
    }
}
