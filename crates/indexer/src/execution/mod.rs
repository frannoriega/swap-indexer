use std::collections::{HashMap, HashSet};

use parser_core::instruction::operation::Operation;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    unionfind::UnionFind,
};
use solana_sdk::pubkey::Pubkey;

#[derive(Clone, Copy, Debug)]
pub struct ExecutionStep {
    pub program_id: Pubkey,
    pub operation: Operation,
    pub outer_idx: u8,
    pub stack_depth: u32,
}

#[derive(Clone, Copy, Debug)]
struct ExecutionNode {
    order: usize,
    program_id: Pubkey,
    operation: Operation,
    // Index of the top-level instruction
    outer_idx: u8,
    stack_depth: u32,
}

type Execution = petgraph::graph::DiGraph<ExecutionNode, Pubkey>;

#[derive(Debug)]
pub struct ExecutionGraph {
    executions: Vec<Execution>,
}

impl ExecutionGraph {
    /**
     * Builds an [ExecutionGraph] from a list of [ExecutionStep]s.
     * This assumes the [ExecutionStep]s are already ordered.
     *
     * NOTE: This algorithm is heuristic. There could be a case where
     * two unrelated executions use the same source accounts, in which case
     * we cannot distinguish them (easily), and they will be grouped
     * into the same [Execution]
     */
    pub fn new(executions: Vec<ExecutionStep>) -> ExecutionGraph {
        // One single graph that will have all the executions.
        // This is a forest DAG: multiple executions will be independent,
        // and will belong to the same graph.
        // At the end of this algorithm, this graph will be split into multiple ones,
        // one per execution.
        let mut graph: Execution = DiGraph::new();
        // We will use this for grouping different executions later.
        // We'll take advantage of the fact that we don't remove anything
        // for the graph, ever; so indices in the graph are essentially the insertion
        // order
        let mut execution_groups: UnionFind<usize> = UnionFind::new(executions.len());

        // The operation nodes associated to a given (unclaimed) destination account.
        // Unclaim = We've seen no operation that had this account as part of it sources account
        let mut unclaimed_dests: HashMap<Pubkey, Vec<NodeIndex>> = HashMap::new();
        let mut instr_grouping: HashMap<u8, NodeIndex> = HashMap::new();

        for (order, exec) in executions.iter().enumerate() {
            let sources = exec.operation.sources();
            let dests = exec.operation.destinations();
            let node = graph.add_node(ExecutionNode {
                order,
                program_id: exec.program_id,
                operation: exec.operation,
                outer_idx: exec.outer_idx,
                stack_depth: exec.stack_depth,
            });

            match instr_grouping.get(&exec.outer_idx) {
                Some(&root) => {
                    execution_groups.union(root.index(), node.index());
                },
                None => {
                    instr_grouping.insert(exec.outer_idx, node);
                },
            };

            for src in sources {
                // If this operation has a source account that was the destination
                // of one or more previous operations, we need to link them in the graph
                if let Some(producers) = unclaimed_dests.remove(&src) {
                    for prod in producers {
                        // We add the edge in the graph
                        graph.add_edge(prod, node, src);
                        // We also group them together into the same execution
                        execution_groups.union(prod.index(), node.index());
                    }
                }
            }

            for dest in dests {
                // Track the account destinations for this particular operation
                unclaimed_dests.entry(dest).or_default().push(node);
            }
        }

        // Split the one big graph into execution-grouped graphs
        let mut groups: HashMap<usize, HashSet<NodeIndex>> = HashMap::new();
        for idx in graph.node_indices() {
            groups
                .entry(execution_groups.find(idx.index()))
                .or_default()
                .insert(idx);
        }

        let executions = groups
            .into_values()
            .map(|nodes| {
                graph.filter_map(|idx, w| nodes.contains(&idx).then_some(*w), |_, w| Some(*w))
            })
            .collect();
        ExecutionGraph { executions }
    }

    pub fn executions(&self) -> &[Execution] {
        &self.executions
    }
}

#[cfg(test)]
mod tests {
    use std::ops::RangeInclusive;

    use super::*;
    use parser_core::instruction::operation::swap::{SwapInOperation, SwapOutOperation};
    use proptest::{
        arbitrary::any,
        prop_oneof,
        strategy::{Just, Strategy},
    };
    #[cfg(test)]
    use proptest::{prop_assert, prop_assert_eq};
    use test_strategy::proptest;
    use utils::proptest::{address, addresses};

    use crate::execution::ExecutionGraph;

    #[derive(Debug, Clone, Copy)]
    enum RawHop {
        SwapIn { amount_in: u64, min_amount_out: u64 },
        SwapOut { max_amount_in: u64, amount_out: u64 }, // Add variants here if you need to test other operations
                                                         // This will work as long as they have one source, one destination
    }

    fn raw_hop() -> impl Strategy<Value = RawHop> {
        prop_oneof![
            (any::<u64>(), any::<u64>()).prop_map(|(amount_in, min_amount_out)| RawHop::SwapIn {
                amount_in,
                min_amount_out
            }),
            (any::<u64>(), any::<u64>()).prop_map(|(max_amount_in, amount_out)| RawHop::SwapOut {
                max_amount_in,
                amount_out
            }) // Add new operations here
        ]
    }

    fn finish(raw: RawHop, src: Pubkey, dest: Pubkey) -> Operation {
        match raw {
            RawHop::SwapIn {
                amount_in,
                min_amount_out,
            } => Operation::SwapIn(SwapInOperation {
                source: src,
                dest,
                amount_in,
                min_amount_out,
            }),
            RawHop::SwapOut {
                max_amount_in,
                amount_out,
            } => Operation::SwapOut(SwapOutOperation {
                source: src,
                dest,
                max_amount_in,
                amount_out,
            }),
            // Add new operations here
        }
    }

    fn chain(len: RangeInclusive<usize>) -> impl Strategy<Value = Vec<ExecutionStep>> {
        len.prop_flat_map(|k| {
            (
                addresses(k + 1).prop_filter("accounts must be different", |accounts| {
                    accounts.iter().collect::<HashSet<_>>().len() == accounts.len()
                }),
                proptest::collection::vec((address(), raw_hop()), k),
            )
        })
        .prop_map(|(path, hops)| {
            path.windows(2)
                .zip(hops)
                .map(|(pair, (program_id, raw))| ExecutionStep {
                    program_id,
                    operation: finish(raw, pair[0], pair[1]),
                    outer_idx: 0,
                    stack_depth: 0,
                })
                .collect()
        })
        .prop_filter("chains must not be empty", |steps: &Vec<_>| {
            !steps.is_empty()
        })
    }

    // Using with branches <= 2 won't return a diamond
    fn diamond(branches: RangeInclusive<usize>) -> impl Strategy<Value = Vec<ExecutionStep>> {
        branches
            .prop_flat_map(|n| {
                (
                    addresses(3).prop_filter("accounts must be different", |accounts| {
                        accounts.iter().collect::<HashSet<_>>().len() == accounts.len()
                    }),
                    proptest::collection::vec((address(), raw_hop()), n),
                    (address(), raw_hop()),
                )
            })
            .prop_map(|(accounts, fan_out_hops, closing_hop)| {
                let root = accounts[0];
                let intermediate = accounts[1];
                let sink = accounts[2];

                let mut steps: Vec<ExecutionStep> = fan_out_hops
                    .into_iter()
                    .map(|(program_id, raw)| ExecutionStep {
                        program_id,
                        operation: finish(raw, root, intermediate),
                        outer_idx: 0,
                        stack_depth: 0,
                    })
                    .collect();

                let (closing_program_id, closing_raw) = closing_hop;
                steps.push(ExecutionStep {
                    program_id: closing_program_id,
                    operation: finish(closing_raw, intermediate, sink),
                    outer_idx: 0,
                    stack_depth: 0,
                });

                steps
            })
            .prop_filter("diamond must have at least one branch", |steps| {
                steps.len() >= 2
            })
    }

    fn interleave(mut groups: Vec<Vec<ExecutionStep>>, tags: Vec<usize>) -> Vec<ExecutionStep> {
        let n = groups.len();
        let mut result = Vec::with_capacity(tags.len());
        for tag in tags {
            for offset in 0..n {
                let idx = (tag + offset) % n;
                if !groups[idx].is_empty() {
                    result.push(groups[idx].remove(0));
                    break;
                }
            }
        }
        result
    }

    fn group() -> impl Strategy<Value = Vec<ExecutionStep>> {
        prop_oneof![chain(0..=10), diamond(2..=5)]
    }

    #[derive(Debug)]
    struct ExecutionsCase {
        steps: Vec<ExecutionStep>,
        group_count: usize,
    }

    fn executions() -> impl Strategy<Value = ExecutionsCase> {
        proptest::collection::vec(group(), 1..=3)
            .prop_flat_map(|mut groups| {
                // Each generated group must look like one top-level instruction,
                // and distinct groups must never collide on outer_idx -- otherwise
                // instr_grouping would wrongly merge unrelated trades, which is
                // exactly the false-positive this field exists to prevent.
                for (idx, group) in groups.iter_mut().enumerate() {
                    for step in group.iter_mut() {
                        step.outer_idx = idx as u8;
                    }
                }
                let total: usize = groups.iter().map(Vec::len).sum();
                let n = groups.len();
                (Just(groups), proptest::collection::vec(0..n, total))
            })
            .prop_map(|(groups, tags)| ExecutionsCase {
                group_count: groups.len(),
                steps: interleave(groups, tags),
            })
    }

    #[test]
    fn empty_executions() {
        let executions = ExecutionGraph::new(vec![]).executions;
        assert!(executions.is_empty());
    }

    #[proptest]
    fn executions_split_into_expected_groups(#[strategy(executions())] case: ExecutionsCase) {
        let total_steps = case.steps.len();
        let graph = ExecutionGraph::new(case.steps);
        let executions = graph.executions();

        // The main thing the union-find grouping exists to guarantee
        prop_assert_eq!(executions.len(), case.group_count);

        // Nothing lost or duplicated while splitting the one big graph apart
        let total_nodes: usize = executions.iter().map(|e| e.node_count()).sum();
        prop_assert_eq!(total_nodes, total_steps);

        for execution in executions {
            // Never a cycle, regardless of shape or interleaving
            prop_assert!(!petgraph::algo::is_cyclic_directed(execution));

            // Each split-off group really is one connected piece, not several
            // unrelated nodes that happened to land in the same Vec slot
            prop_assert_eq!(petgraph::algo::connected_components(execution), 1);
        }
    }

    #[proptest]
    fn chain_is_one_connected_path(#[strategy(chain(1..=8))] steps: Vec<ExecutionStep>) {
        let len = steps.len();
        let graph = ExecutionGraph::new(steps);
        let executions = graph.executions();
        prop_assert_eq!(executions.len(), 1);
        let execution = &executions[0];
        prop_assert_eq!(execution.node_count(), len);
        prop_assert_eq!(execution.edge_count(), len - 1);
        prop_assert_eq!(
            execution.externals(petgraph::Direction::Incoming).count(),
            1
        );
        prop_assert_eq!(
            execution.externals(petgraph::Direction::Outgoing).count(),
            1
        );
    }

    #[proptest]
    fn diamond_fans_in_correctly(#[strategy(diamond(2..=5))] steps: Vec<ExecutionStep>) {
        let n = steps.len() - 1;
        let graph = ExecutionGraph::new(steps);
        let executions = graph.executions();
        prop_assert_eq!(executions.len(), 1);
        let execution = &executions[0];
        prop_assert_eq!(execution.node_count(), n + 1);
        prop_assert_eq!(execution.edge_count(), n);
        prop_assert_eq!(
            execution.externals(petgraph::Direction::Incoming).count(),
            n
        );
        prop_assert_eq!(
            execution.externals(petgraph::Direction::Outgoing).count(),
            1
        );
    }
}
