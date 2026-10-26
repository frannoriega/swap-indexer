pub mod swap;
pub mod transfer;

use solana_sdk::pubkey::Pubkey;

use crate::instruction::operation::{
    swap::{SwapInOperation, SwapOutOperation},
    transfer::TransferOperation,
};

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Operation {
    SwapIn(SwapInOperation),
    SwapOut(SwapOutOperation),
    Transfer(TransferOperation),
}

impl Operation {
    pub fn sources(&self) -> Vec<Pubkey> {
        match self {
            Operation::SwapIn(s) => vec![s.source],
            Operation::SwapOut(s) => vec![s.source],
            Operation::Transfer(s) => vec![s.source],
        }
    }

    pub fn destinations(&self) -> Vec<Pubkey> {
        match self {
            Operation::SwapIn(s) => vec![s.dest],
            Operation::SwapOut(s) => vec![s.dest],
            Operation::Transfer(s) => vec![s.dest],
        }
    }
}
