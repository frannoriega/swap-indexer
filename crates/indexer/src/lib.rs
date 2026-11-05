pub mod accounts;
pub mod call_tree;
pub mod deltas;
pub mod domain;
pub mod execution;
pub mod interpreter;

use std::collections::HashMap;

use anyhow::anyhow;
use parser_core::instruction::{InstructionParser, Parsed, Resource};
use solana_sdk::pubkey::Pubkey;
use types::{Instruction, RawTx};

use crate::call_tree::CallTree;
use crate::execution::ExecutionGraph;

pub struct ProcessedTx<T> {
    pub call_tree: CallTree<T>,
    pub flow_graph: ExecutionGraph<Parsed<T>>,
}

pub struct Indexer<T> {
    parsers: HashMap<Pubkey, Box<dyn InstructionParser<T> + Send + Sync>>,
}

pub(crate) fn build_processed<T: Resource + Default + Copy>(
    parsed: Vec<Parsed<T>>,
) -> ProcessedTx<T> {
    ProcessedTx {
        call_tree: CallTree::new(parsed.clone()),
        flow_graph: ExecutionGraph::new(parsed),
    }
}

impl<T: Resource + Default + Copy> Indexer<T> {
    pub fn process(&self, tx: &RawTx) -> Result<ProcessedTx<T>, anyhow::Error> {
        if let Some(err) = &tx.status {
            return Err(anyhow!("Transaction failed: {:}", err));
        }

        let parsed: Vec<Parsed<T>> = tx
            .instructions
            .iter()
            .map(|instr| self.parse_one(instr))
            .collect::<Result<_, _>>()?;

        Ok(build_processed(parsed))
    }

    pub(crate) fn parse_one(&self, instr: &Instruction) -> Result<Parsed<T>, anyhow::Error> {
        let operation = match self.parsers.get(&instr.program_id) {
            Some(parser) => parser.parse(instr)?.unwrap_or_default(),
            None => T::default(),
        };
        Ok(Parsed {
            operation,
            program_id: instr.program_id,
            outer_idx: instr.outer_idx,
            stack_depth: instr.stack_depth,
        })
    }
}

#[derive(Default)]
pub struct IndexerBuilder<T> {
    parsers: HashMap<Pubkey, Box<dyn InstructionParser<T> + Send + Sync>>,
}

impl<T> IndexerBuilder<T> {
    pub fn new() -> Self {
        IndexerBuilder {
            parsers: HashMap::new(),
        }
    }

    pub fn with_boxed_parser(
        mut self,
        program_id: Pubkey,
        parser: Box<dyn InstructionParser<T> + Send + Sync>,
    ) -> Self {
        self.parsers.insert(program_id, parser);
        self
    }

    pub fn with_parser(
        self,
        program_id: Pubkey,
        parser: impl InstructionParser<T> + Send + Sync + 'static,
    ) -> Self {
        self.with_boxed_parser(program_id, Box::new(parser))
    }

    pub fn build(self) -> Indexer<T> {
        Indexer {
            parsers: self.parsers,
        }
    }
}
