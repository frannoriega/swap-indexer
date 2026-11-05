use parser_core::instruction::Linkable;
use solana_sdk::pubkey::Pubkey;

use crate::execution::ExecutionGraph;

pub trait Interpreter<'a, I: Linkable + Copy, O> {
    fn interpret(graph: &'a ExecutionGraph<I>) -> O;
}

pub trait Event {
    fn program_id(&self) -> Pubkey;

    fn describe(&self) -> String;
}
