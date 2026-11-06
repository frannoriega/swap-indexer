use std::collections::HashMap;

use indexer::accounts::{TokenResolver, token_accounts};
use indexer::domain::Domain;
use indexer::interpreter::Interpreter;
use parser_core::instruction::InstructionParser;
use plain_trade_domain::PlainTradeDomain;
use solana_sdk::pubkey::Pubkey;
use trade_interpreter::TradeInterpreter;
use trade_interpreter::event::summarize;
use trade_interpreter::resolver::{ResolvedTradeEvent, resolve};
use trade_model::Operation;
use types::RawTx;

pub struct ResolvedTradeDomain<T: TokenResolver> {
    inner_domain: PlainTradeDomain,
    resolver: T,
}

impl<T: TokenResolver> ResolvedTradeDomain<T> {
    pub fn new(resolver: T) -> Self {
        ResolvedTradeDomain {
            inner_domain: PlainTradeDomain,
            resolver,
        }
    }
}

impl<T: TokenResolver> Domain for ResolvedTradeDomain<T> {
    type Payload = Operation;
    type Event = ResolvedTradeEvent;

    fn parsers(&self) -> HashMap<Pubkey, Box<dyn InstructionParser<Self::Payload> + Send + Sync>> {
        self.inner_domain.parsers()
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
            .filter_map(|trade| {
                summarize(&trade, &processed.call_tree, &accounts)
                    .map(|e| resolve(&e, &self.resolver))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use indexer::ProcessedTx;
    use indexer::accounts::{StaticTokenResolver, TokenInfo};
    use indexer::call_tree::CallTree;
    use indexer::execution::ExecutionGraph;
    use solana_sdk::pubkey;
    use trade_model::{ParsedInstruction, swap::SwapInOperation, transfer::TransferOperation};
    use types::TokenBalance;

    use super::*;

    const WSOL_TOKEN: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    const SPL_TOKEN_2022: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
    const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

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

    /// `parsers()` is pure delegation to [PlainTradeDomain] - this just
    /// pins down that the delegation actually happens, not the mapping
    /// itself (which `plain-trade-domain`'s own tests already own).
    #[test]
    fn delegates_program_ids_to_the_plain_domain() {
        let domain = ResolvedTradeDomain::new(StaticTokenResolver::new(HashMap::new()));
        let ids: HashSet<_> = domain.parsers().keys().copied().collect();
        assert_eq!(
            ids,
            HashSet::from([RAYDIUM_AMM_V4, WSOL_TOKEN, SPL_TOKEN_2022])
        );
    }

    /// Exercises the one thing this crate adds on top of `summarize` (which
    /// `plain-trade-domain` already covers): that each leg's mint is looked
    /// up through the resolver, known mints get a symbol and unknown ones
    /// degrade to `None` rather than erroring.
    #[test]
    fn events_resolves_known_mints_and_leaves_unknown_ones_unresolved() {
        let wallet_src = Pubkey::new_unique();
        let pool = Pubkey::new_unique();
        let known_mint = Pubkey::new_unique();
        let unknown_mint = Pubkey::new_unique();

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

        let tx = RawTx {
            pre_token_balances: vec![
                TokenBalance {
                    account: wallet_src,
                    mint: known_mint,
                    owner: None,
                    amount: 0,
                    decimals: 6,
                },
                TokenBalance {
                    account: pool,
                    mint: unknown_mint,
                    owner: None,
                    amount: 0,
                    decimals: 9,
                },
            ],
            ..Default::default()
        };

        let mut known = HashMap::new();
        known.insert(
            known_mint,
            TokenInfo {
                symbol: "USDC".to_owned(),
                name: Some("USDC".to_owned()),
            },
        );
        let resolver = StaticTokenResolver::new(known);
        let domain = ResolvedTradeDomain::new(resolver);

        let events = domain.events(&tx, &processed);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].input[&wallet_src].symbol,
            Some("USDC".to_owned())
        );
        assert_eq!(events[0].output[&pool].symbol, None);
    }
}
