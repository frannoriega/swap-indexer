pub mod swap;
pub mod transfer;

use parser_core::instruction::{Parsed, Resource};
use solana_sdk::pubkey::Pubkey;

use crate::{
    swap::{SwapInOperation, SwapOutOperation},
    transfer::TransferOperation,
};

#[derive(PartialEq, Debug, Default, Clone, Copy)]
pub enum Operation {
    SwapIn(SwapInOperation),
    SwapOut(SwapOutOperation),
    Transfer(TransferOperation),
    #[default]
    Unknown,
}

impl Resource for Operation {
    fn consumes(&self) -> Vec<Pubkey> {
        match self {
            Operation::SwapIn(s) => vec![s.source],
            Operation::SwapOut(s) => vec![s.source],
            Operation::Transfer(s) => vec![s.source],
            Operation::Unknown => vec![],
        }
    }

    fn produces(&self) -> Vec<Pubkey> {
        match self {
            Operation::SwapIn(s) => vec![s.dest],
            Operation::SwapOut(s) => vec![s.dest],
            Operation::Transfer(s) => vec![s.dest],
            Operation::Unknown => vec![],
        }
    }
}

pub type ParsedInstruction = Parsed<Operation>;
