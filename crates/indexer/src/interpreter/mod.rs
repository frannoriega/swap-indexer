pub mod trade;

use parser_core::instruction::Linkable;

use crate::execution::ExecutionGraph;

pub trait Interpreter<'a, I: Linkable + Copy, O> {
    fn interpret(graph: &'a ExecutionGraph<I>) -> O;
}
