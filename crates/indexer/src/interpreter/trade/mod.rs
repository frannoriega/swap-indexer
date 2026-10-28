use parser_core::instruction::{ParsedInstruction, operation::Operation};
use petgraph::{Direction, graph::NodeIndex};

use crate::{
    execution::{Execution, ExecutionGraph},
    interpreter::Interpreter,
};

pub type TradeGraph = ExecutionGraph<ParsedInstruction>;

pub struct Trade<'a>(&'a Execution<ParsedInstruction>);

impl<'a> Trade<'a> {
    pub fn graph(&self) -> &'a Execution<ParsedInstruction> {
        self.0
    }

    pub fn entry(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.0.externals(Direction::Incoming)
    }

    pub fn exit(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.0.externals(Direction::Outgoing)
    }
}

pub struct TradeInterpreter;

impl<'a> Interpreter<'a, ParsedInstruction, Vec<Trade<'a>>> for TradeInterpreter {
    fn interpret(graph: &'a TradeGraph) -> Vec<Trade<'a>> {
        graph
            .executions()
            .iter()
            .filter(|component| {
                component.node_weights().any(|n| {
                    matches!(
                        n.weight.operation,
                        Operation::SwapIn(_) | Operation::SwapOut(_)
                    )
                })
            })
            .map(Trade)
            .collect()
    }
}
