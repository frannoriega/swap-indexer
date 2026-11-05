use std::collections::{HashMap, HashSet};

use indexer::{accounts::AccountInfo, call_tree::CallTree, interpreter::Event};
use parser_core::instruction::Resource;
use petgraph::graph::NodeIndex;
use solana_sdk::pubkey::Pubkey;
use trade_model::Operation;

use crate::Trade;

/// A balance movement. Note that this doesn't represent the direction of the movement,
/// just that an `account` moved `amount` of `token`s.
///
/// `account` is a plain Solana account address (a [Pubkey]); usually the
/// SPL token account that held the balance, but for native SOL it's the
/// wallet's own system account instead. Whether it turned out to be a
/// *known* token account is exactly what `token` (below) records.
pub struct TokenAmount {
    pub account: Pubkey,
    pub token: Option<AccountInfo>,
    pub amount: u64,
}

/// Any balance movement, keyed by account.
/// This is the final merge of all movements related to the account, which
/// is why a map is needed: we only want (and allow to represent) one token amount
/// (balance change) per account
pub type Legs = HashMap<Pubkey, TokenAmount>;

pub struct TradeEvent {
    pub program_id: Pubkey,
    pub input: Legs,
    pub output: Legs,
}

impl Event for TradeEvent {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn describe(&self) -> String {
        let side = |legs: &Legs| {
            legs.values()
                .map(|t| format!("{} of {}", t.amount, t.account))
                .collect::<Vec<_>>()
                .join(" + ")
        };
        format!("Swap: {} -> {}", side(&self.input), side(&self.output))
    }
}

/// Here we have the `candidates`, which are operations in the `CallTree` that carry a balance
/// (the `u64`) and are associated with a given `account` (see [legs_for]).
/// We need to boil that down to a single amount, but there could be more than one instruction
/// touching that exact account:
/// - The same real movement observed at two CPI depths (a `SwapIn` and the inner `Transfer` it
///   triggers both touch the account directly; only the `Transfer` is exact): these should collapse
///   to one reading, not be added together.
/// - A diamond (aggregators): one instruction fanning a trade out across several pools and
///   recombining it. These are genuinely independent legs and should be summed, not picked between.
///
/// So, we only sum up the innermost CPIs: we look at the candidates (again, the operations) and
/// discard them if they are an ancestor of another candidate (meaning, that other candidate is
/// "deeper" in the call stack).
/// Siblings should survive as well, and be added together.
fn resolve_group(candidates: HashMap<NodeIndex, u64>, call_tree: &CallTree<Operation>) -> u64 {
    let mut ancestors = HashSet::new();
    for candidate in candidates.keys() {
        if ancestors.contains(candidate) {
            continue;
        }
        for ancestor in call_tree.ancestors(*candidate) {
            if candidates.contains_key(&ancestor) {
                ancestors.insert(ancestor);
            }
        }
    }
    candidates
        .iter()
        .filter_map(|(k, v)| (!ancestors.contains(k)).then_some(v))
        .sum()
}

/// Represents a balance shift (transfer affecting the `account`) that
/// occurred at the `idx` in a [CallTree]
struct BalanceShift {
    idx: NodeIndex,
    account: Pubkey,
    amount: u64,
}

/// Group a list of operations that move balances to the account they affect, then resolve only
/// the ones we want.
/// Since there might be more than one operation moving the same amount (like `SwapIn` and
/// `Transfer`), we only care about the innermost CPIs (innermost operations). See [resolve_group]
/// for more info.
fn legs_for(
    candidates: Vec<BalanceShift>,
    call_tree: &CallTree<Operation>,
) -> HashMap<Pubkey, u64> {
    let mut by_account: HashMap<Pubkey, HashMap<NodeIndex, u64>> = HashMap::new();
    for balance_shift in candidates {
        by_account
            .entry(balance_shift.account)
            .or_default()
            .insert(balance_shift.idx, balance_shift.amount);
    }

    by_account
        .into_iter()
        .map(|(account, group)| (account, resolve_group(group, call_tree)))
        .collect()
}

/// Resolve the [TokenAmount] for a given account and amount.
/// This essentially just checks whether `account` is one we recognize as an
/// SPL token account - i.e. whether it shows up in `accounts` - and if so,
/// attaches the `mint`/`decimals` we have for it. If not (e.g. it's a
/// wallet's native-SOL system account), `token` is left `None`.
fn token_amount(
    account: Pubkey,
    amount: u64,
    accounts: &HashMap<Pubkey, AccountInfo>,
) -> TokenAmount {
    let info = accounts.get(&account);
    TokenAmount {
        account,
        token: info.map(|i| AccountInfo {
            mint: i.mint,
            decimals: i.decimals,
        }),
        amount,
    }
}

/// Returns the shifts in balances for all accounts on both sides of a [Trade] (input and output).
/// Both sides accept the same two kinds of exact reading: the input side takes a `SwapIn`'s
/// `amount_in` or a `Transfer`'s `amount`; the output side takes a `SwapOut`'s `amount_out` or
/// a `Transfer`'s `amount`.
/// Everything else (e.g. `SwapOut` on the input side, whose `max_amount_in` is only a bound, not an
/// exact value) contributes nothing to that side.
///
/// This requires that the [Trade] (and its graph) is built from the [CallTree], and that the
/// [CallTree] is built using the original, ordered instruction list.
/// That way, we can use the `node.order` information to get the index of the operation's original
/// node in the [CallTree] (since remember, a [Trade] contains a subgraph of [CallTree], so their
/// indices won't match).
///
/// Returns `None` if *either* side ends up with no resolvable candidate at all
/// (having more than one resolved account on a side is fine; it's only an
/// empty side that fails).
fn legs(
    trade: &Trade<'_>,
    call_tree: &CallTree<Operation>,
) -> Option<(HashMap<Pubkey, u64>, HashMap<Pubkey, u64>)> {
    let graph = trade.graph();

    // Check the shifts in balances by account, both from the entry and exit nodes.
    let input_candidates = trade
        .entry()
        .flat_map(|idx| {
            let node = &graph[idx];
            let amount = match &node.weight.operation {
                Operation::SwapIn(s) => s.amount_in,
                Operation::Transfer(t) => t.amount,
                _ => return vec![],
            };
            node.weight
                .operation
                .consumes()
                .into_iter()
                .map(|account| BalanceShift {
                    idx: NodeIndex::new(node.order),
                    account,
                    amount,
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let output_candidates = trade
        .exit()
        .flat_map(|idx| {
            let node = &graph[idx];
            let amount = match &node.weight.operation {
                Operation::SwapOut(s) => s.amount_out,
                Operation::Transfer(t) => t.amount,
                _ => return vec![],
            };
            node.weight
                .operation
                .produces()
                .into_iter()
                .map(|account| BalanceShift {
                    idx: NodeIndex::new(node.order),
                    account,
                    amount,
                })
                .collect::<Vec<_>>()
        })
        .collect();

    // Compute the final map of `account` -> balance shift
    let input = legs_for(input_candidates, call_tree);
    let output = legs_for(output_candidates, call_tree);

    if input.is_empty() || output.is_empty() {
        return None;
    }

    Some((input, output))
}

/// Flattens a [Trade] into a concrete [TradeEvent], which is the only place a
/// trade's balance movement becomes plain numbers instead of something we
/// have to walk the graph to find.
///
/// `program_id` is read off the shallowest node in the component (ties
/// broken by the earliest `order`), which is the instruction that started this
/// trade, not any particular swap found inside it. This is deliberately
/// generic: it works even when that instruction is `Operation::Unknown`
/// (an aggregator we have no parser for), because every CPI it triggers
/// necessarily inherits its `outer_idx` and is therefore strictly deeper, and
/// we only need to know *that* it started the trade, never *what* it did.
///
/// A diamond (one program splitting this trade across several pools and
/// recombining it) is handled correctly here, and so is a trade with more
/// than one input or output resource (see [Legs] and [resolve_group]).
/// Returns `None` only when a side has no exact amount at all.
pub fn summarize(
    trade: &Trade<'_>,
    call_tree: &CallTree<Operation>,
    accounts: &HashMap<Pubkey, AccountInfo>,
) -> Option<TradeEvent> {
    let program_id = trade
        .graph()
        .node_weights()
        .min_by_key(|n| (n.weight.stack_depth, n.order))
        .map(|n| n.weight.program_id)?;

    let (input_amounts, output_amounts) = legs(trade, call_tree)?;

    let to_legs = |amounts: HashMap<Pubkey, u64>| -> Legs {
        amounts
            .into_iter()
            .map(|(account, amount)| (account, token_amount(account, amount, accounts)))
            .collect()
    };

    Some(TradeEvent {
        program_id,
        input: to_legs(input_amounts),
        output: to_legs(output_amounts),
    })
}

#[cfg(test)]
mod tests {
    use indexer::execution::ExecutionGraph;
    use solana_sdk::pubkey::Pubkey;
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

    /// A `SwapIn` and the inner `Transfer` CPI it triggers both touch the
    /// wallet's source account directly (the `SwapIn`'s own fields, and the
    /// literal token movement)
    /// This must collapse to one amount, not double-count.
    #[test]
    fn collapses_cpi_duplicate_into_one_amount() {
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

        let call_tree = CallTree::new(instructions.clone());
        let graph = ExecutionGraph::<ParsedInstruction>::new(instructions);
        assert_eq!(graph.executions().len(), 1);
        let trade = Trade(&graph.executions()[0]);

        let (input, _) = legs(&trade, &call_tree).unwrap();
        assert_eq!(input, HashMap::from([(wallet_src, 600)]));
    }

    /// Two independent `SwapIn`s, both triggered by the same router (same
    /// `outer_idx`), each sourcing from the wallet and each delivering into
    /// the same destination account via its own inner `Transfer`. Neither
    /// branch is a CPI descendant of the other, so both amounts must
    /// survive and be summed.
    #[test]
    fn sums_independent_sibling_legs() {
        let wallet_src = Pubkey::new_unique();
        let wallet_dst = Pubkey::new_unique();
        let pool_a = Pubkey::new_unique();
        let pool_b = Pubkey::new_unique();

        let instructions = vec![
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_src,
                    dest: pool_a,
                    amount_in: 600,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: pool_a,
                    dest: wallet_dst,
                    mint: None,
                    amount: 100,
                    authority: pool_a,
                }),
                0,
                2,
            ),
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_src,
                    dest: pool_b,
                    amount_in: 400,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: pool_b,
                    dest: wallet_dst,
                    mint: None,
                    amount: 50,
                    authority: pool_b,
                }),
                0,
                2,
            ),
        ];

        let call_tree = CallTree::new(instructions.clone());
        let graph = ExecutionGraph::<ParsedInstruction>::new(instructions);
        assert_eq!(graph.executions().len(), 1);
        let trade = Trade(&graph.executions()[0]);

        let (input, output) = legs(&trade, &call_tree).unwrap();
        assert_eq!(input, HashMap::from([(wallet_src, 1000)])); // 600 (pool A) + 400 (pool B)
        assert_eq!(output, HashMap::from([(wallet_dst, 150)])); // 100 (pool A) + 50 (pool B)
    }

    /// Branch 1 is a `SwapIn` plus its own duplicate-reading `Transfer` CPI (must collapse to
    /// one), sitting alongside an entirely independent branch 2 sharing the same input account
    /// (must still be summed in, not excluded by branch 1's collapse).
    /// Exercises that collapsing  one sibling's redundant reading doesn't accidentally swallow a
    /// genuinely independent one sharing the same account.
    #[test]
    fn collapses_duplicate_while_summing_independent_sibling() {
        let wallet_src = Pubkey::new_unique();
        let wallet_dst = Pubkey::new_unique();
        let pool_a = Pubkey::new_unique();
        let pool_b = Pubkey::new_unique();

        let instructions = vec![
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_src,
                    dest: pool_a,
                    amount_in: 600,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            // Duplicate reading of the exact same leg as the SwapIn above -
            // same source and dest - must collapse away, not add to it.
            parsed(
                Operation::Transfer(TransferOperation {
                    source: wallet_src,
                    dest: pool_a,
                    mint: None,
                    amount: 600,
                    authority: wallet_src,
                }),
                0,
                2,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: pool_a,
                    dest: wallet_dst,
                    mint: None,
                    amount: 100,
                    authority: pool_a,
                }),
                0,
                2,
            ),
            // Independent sibling branch, sharing the same input and
            // output accounts as branch 1 but caused by neither of its
            // instructions - must survive and be summed in.
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_src,
                    dest: pool_b,
                    amount_in: 400,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: pool_b,
                    dest: wallet_dst,
                    mint: None,
                    amount: 50,
                    authority: pool_b,
                }),
                0,
                2,
            ),
        ];

        let call_tree = CallTree::new(instructions.clone());
        let graph = ExecutionGraph::<ParsedInstruction>::new(instructions);
        assert_eq!(graph.executions().len(), 1);
        let trade = Trade(&graph.executions()[0]);

        let (input, output) = legs(&trade, &call_tree).unwrap();
        assert_eq!(input, HashMap::from([(wallet_src, 1000)])); // 600 (collapsed) + 400 (sibling)
        assert_eq!(output, HashMap::from([(wallet_dst, 150)])); // 100 + 50
    }

    /// A single trade with two distinct input tokens combined into one
    /// output (ie, a "zap" consolidating SOL and USDC into ETH in one
    /// instruction). Neither input shares an account with the other, so
    /// `legs_for` groups them into two separate entries in `input` instead
    /// of merging or rejecting them the way it would if they shared one
    /// account.
    #[test]
    fn supports_more_than_one_input_resource() {
        let wallet_sol = Pubkey::new_unique();
        let wallet_usdc = Pubkey::new_unique();
        let wallet_eth = Pubkey::new_unique();
        let pool_sol = Pubkey::new_unique();
        let pool_usdc = Pubkey::new_unique();

        let instructions = vec![
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_sol,
                    dest: pool_sol,
                    amount_in: 2_000_000_000,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: pool_sol,
                    dest: wallet_eth,
                    mint: None,
                    amount: 10,
                    authority: pool_sol,
                }),
                0,
                2,
            ),
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_usdc,
                    dest: pool_usdc,
                    amount_in: 500_000_000,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: pool_usdc,
                    dest: wallet_eth,
                    mint: None,
                    amount: 5,
                    authority: pool_usdc,
                }),
                0,
                2,
            ),
        ];

        let call_tree = CallTree::new(instructions.clone());
        let graph = ExecutionGraph::<ParsedInstruction>::new(instructions);
        assert_eq!(graph.executions().len(), 1);
        let trade = Trade(&graph.executions()[0]);

        let (input, output) = legs(&trade, &call_tree).unwrap();
        assert_eq!(
            input,
            HashMap::from([(wallet_sol, 2_000_000_000), (wallet_usdc, 500_000_000)])
        );
        assert_eq!(output, HashMap::from([(wallet_eth, 15)]));
    }

    /// A full diamond nested under a real, unrecognized router instruction
    /// (e.g. an aggregator we have no parser for): the router's own
    /// Operation is Unknown, and both SwapIns are its actual CPI children,
    /// not just same-outer_idx siblings. summarize() must still (a) sum
    /// both branches correctly and (b) label the trade with the router's
    /// program_id - never the underlying pools' - without ever
    /// recognizing what the router itself does.
    #[test]
    fn labels_trade_by_its_starting_instruction_even_when_unknown() {
        let router_program = Pubkey::new_unique();
        let wallet_src = Pubkey::new_unique();
        let wallet_dst = Pubkey::new_unique();
        let pool_a = Pubkey::new_unique();
        let pool_b = Pubkey::new_unique();

        let instructions = vec![
            ParsedInstruction {
                operation: Operation::Unknown,
                program_id: router_program,
                outer_idx: 0,
                stack_depth: 0,
            },
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_src,
                    dest: pool_a,
                    amount_in: 600,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: pool_a,
                    dest: wallet_dst,
                    mint: None,
                    amount: 100,
                    authority: pool_a,
                }),
                0,
                2,
            ),
            parsed(
                Operation::SwapIn(SwapInOperation {
                    source: wallet_src,
                    dest: pool_b,
                    amount_in: 400,
                    min_amount_out: 0,
                }),
                0,
                1,
            ),
            parsed(
                Operation::Transfer(TransferOperation {
                    source: pool_b,
                    dest: wallet_dst,
                    mint: None,
                    amount: 50,
                    authority: pool_b,
                }),
                0,
                2,
            ),
        ];

        let call_tree = CallTree::new(instructions.clone());
        let graph = ExecutionGraph::<ParsedInstruction>::new(instructions);
        assert_eq!(graph.executions().len(), 1);
        let trade = Trade(&graph.executions()[0]);

        let event = summarize(&trade, &call_tree, &HashMap::new()).unwrap();
        assert_eq!(event.program_id, router_program);
        assert_eq!(event.input[&wallet_src].amount, 1000);
        assert_eq!(event.output[&wallet_dst].amount, 150);
    }

    /// There is currently no guard against a `call_tree` built from a
    /// different instruction ordering than the one `node.order` was
    /// derived from (see [legs]'s doc comment on that coupling). This test
    /// doesn't show the code "catching" that - it shows the actual failure
    /// mode instead, so a future fix has a concrete wrong-answer to fix: a
    /// `call_tree` built from a reordered copy of the same instructions
    /// doesn't error, it just makes `resolve_group` silently double-count
    /// a reading that should have collapsed.
    ///
    /// If this test ever starts failing because `resolve_group` now
    /// panics, errors, or otherwise refuses to run on a mismatched
    /// `call_tree`, that's progress, not a regression - update the
    /// assertion to match the new (better) behavior.
    #[test]
    fn broken_call_tree_silently_double_counts_instead_of_collapsing() {
        let wallet_src = Pubkey::new_unique();
        let pool = Pubkey::new_unique();

        // Same two instructions as `collapses_cpi_duplicate_into_one_amount`:
        // a SwapIn and the inner Transfer CPI it triggers, both reading the
        // same 600-unit movement out of `wallet_src`.
        let swap_in = parsed(
            Operation::SwapIn(SwapInOperation {
                source: wallet_src,
                dest: pool,
                amount_in: 600,
                min_amount_out: 0,
            }),
            0,
            1,
        );
        let transfer = parsed(
            Operation::Transfer(TransferOperation {
                source: wallet_src,
                dest: pool,
                mint: None,
                amount: 600,
                authority: wallet_src,
            }),
            0,
            2,
        );

        // `node.order` for these two candidates is fixed by the ORIGINAL,
        // correctly-ordered list: swap_in is order 0, transfer is order 1.
        let candidates = HashMap::from([(NodeIndex::new(0), 600u64), (NodeIndex::new(1), 600u64)]);

        // A call_tree built from that same, correctly-ordered list sees
        // the real parent/child relationship and collapses the duplicate.
        let correct_tree = CallTree::new(vec![swap_in.clone(), transfer.clone()]);
        assert_eq!(resolve_group(candidates.clone(), &correct_tree), 600);

        // A call_tree built from a REORDERED copy of the same two
        // instructions is a perfectly valid, perfectly innocent-looking
        // CallTree - but transfer is now processed before swap_in, so
        // CallTree::new never sees transfer as swap_in's child: both
        // become independent roots. `candidates` still uses the NodeIndex
        // values from the original order, so they now point at the wrong
        // nodes in this tree, and the ancestor relationship that should
        // have collapsed them has vanished entirely.
        let broken_tree = CallTree::new(vec![transfer, swap_in]);
        assert_eq!(resolve_group(candidates, &broken_tree), 1200);
    }
}
