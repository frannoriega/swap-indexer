use std::collections::HashMap;

use indexer::accounts::token_accounts;
use indexer::domain::Domain;
use indexer::interpreter::Interpreter;
use parser_core::instruction::InstructionParser;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use trade_interpreter::TradeInterpreter;
use trade_interpreter::event::{TradeEvent, summarize};
use trade_model::Operation;
use trade_parser_raydium::RaydiumParser;
use trade_parser_spl_token::SplTokenParser;
use types::RawTx;

const WSOL_TOKEN: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const SPL_TOKEN_2022: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

pub struct PlainTradeDomain;

impl Domain for PlainTradeDomain {
    type Payload = Operation;
    type Event = TradeEvent;

    fn parsers(&self) -> HashMap<Pubkey, Box<dyn InstructionParser<Self::Payload> + Send + Sync>> {
        let mut parsers: HashMap<Pubkey, Box<dyn InstructionParser<Operation> + Send + Sync>> =
            HashMap::new();
        parsers.insert(RAYDIUM_AMM_V4, Box::new(RaydiumParser));
        parsers.insert(WSOL_TOKEN, Box::new(SplTokenParser));
        parsers.insert(SPL_TOKEN_2022, Box::new(SplTokenParser));
        parsers
    }

    fn events(
        &self,
        tx: &RawTx,
        processed: &indexer::ProcessedTx<Self::Payload>,
    ) -> Vec<Self::Event> {
        let accounts = token_accounts(tx);
        TradeInterpreter::interpret(&processed.flow_graph)
            .into_iter()
            // TODO: Log dropped trades
            .filter_map(|trade| summarize(&trade, &processed.call_tree, &accounts))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use indexer::ProcessedTx;
    use indexer::call_tree::CallTree;
    use indexer::execution::ExecutionGraph;
    use trade_model::{ParsedInstruction, swap::SwapInOperation, transfer::TransferOperation};

    use super::*;

    fn parsed(operation: Operation, outer_idx: u8, stack_depth: u32) -> ParsedInstruction {
        ParsedInstruction {
            operation,
            program_id: Pubkey::new_unique(),
            outer_idx,
            stack_depth,
        }
    }

    fn processed(instructions: Vec<ParsedInstruction>) -> ProcessedTx<Operation> {
        ProcessedTx {
            call_tree: CallTree::new(instructions.clone()),
            flow_graph: ExecutionGraph::new(instructions),
        }
    }

    #[test]
    fn claims_exactly_its_three_program_ids() {
        let ids: HashSet<_> = PlainTradeDomain.parsers().keys().copied().collect();
        assert_eq!(
            ids,
            HashSet::from([RAYDIUM_AMM_V4, WSOL_TOKEN, SPL_TOKEN_2022])
        );
    }

    /// End-to-end through `events()`: a `SwapIn` and the `Transfer` CPI it
    /// triggers must collapse into one `TradeEvent`, same invariant
    /// `trade_interpreter::event::summarize` itself is tested against, but
    /// here exercised through the full `Domain` pipeline this crate adds
    /// (`token_accounts` + `TradeInterpreter::interpret` + `summarize`).
    #[test]
    fn events_summarizes_a_simple_swap() {
        let wallet_src = Pubkey::new_unique();
        let pool = Pubkey::new_unique();

        let instructions = vec![
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_src,
                    dest: pool,
                    amount_in: 600,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: wallet_src,
                    dest: pool,
                    mint: None,
                    amount: 600,
                    authority: wallet_src,
                }),
                0,
                2,
            ),
        ];

        let processed = processed(instructions);
        let tx = RawTx::default();

        let events = PlainTradeDomain.events(&tx, &processed);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].input[&wallet_src].amount, 600);
        assert_eq!(events[0].output[&pool].amount, 600);
    }

    #[test]
    fn events_is_empty_when_no_trade_is_found() {
        let processed = processed(vec![parsed(Operation::Unknown, 0, 0)]);
        let tx = RawTx::default();
        assert!(PlainTradeDomain.events(&tx, &processed).is_empty());
    }
}
