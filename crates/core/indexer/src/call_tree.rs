use std::iter::successors;

use parser_core::instruction::Parsed;
use petgraph::{
    Direction,
    graph::{DiGraph, NodeIndex},
};

pub struct CallTree<T> {
    tree: DiGraph<Parsed<T>, ()>,
    roots: Vec<NodeIndex>,
}

impl<T: Clone> CallTree<T> {
    /**
     * Builds the literal instruction call tree for a transaction, from
     * every [ParsedInstruction] including
     * `Unknown` placeholders. One root per
     * top-level instruction.
     */
    pub fn new(instructions: Vec<Parsed<T>>) -> CallTree<T> {
        let mut tree = DiGraph::new();
        let mut roots = Vec::new();
        let mut stack: Vec<(u32, NodeIndex)> = Vec::new();

        for instr in instructions {
            let node = tree.add_node(instr.clone());
            while stack
                .last()
                .is_some_and(|(depth, _)| *depth >= instr.stack_depth)
            {
                stack.pop();
            }
            match stack.last() {
                Some((_, parent)) => {
                    tree.add_edge(*parent, node, ());
                },
                None => roots.push(node),
            }
            stack.push((instr.stack_depth, node));
        }

        CallTree { tree, roots }
    }

    pub fn roots(&self) -> &[NodeIndex] {
        &self.roots
    }

    pub fn tree(&self) -> &DiGraph<Parsed<T>, ()> {
        &self.tree
    }

    /// Traverse the graph, starting from the node (not included) towards the root (included).
    /// The [CallTree] is a forest, so we know the node is part of one of the trees
    pub fn ancestors(&self, node: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
        successors(Some(node), move |&n| {
            self.tree.neighbors_directed(n, Direction::Incoming).next()
        })
        .skip(1)
    }

    /// Is `ancestor` part of the `node`s path towards the root?
    pub fn is_ancestor_of(&self, ancestor: NodeIndex, node: NodeIndex) -> bool {
        self.ancestors(node).any(|a| a == ancestor)
    }
}

#[cfg(test)]
mod tests {
    use proptest::{prelude::*, prop_assert_eq};
    use solana_sdk::pubkey::Pubkey;
    use test_strategy::proptest;
    use trade_model::Operation;
    use utils::proptest::addresses;

    use super::*;

    fn arbitrary_instructions() -> impl Strategy<Value = Vec<Parsed<Operation>>> {
        (1..=20usize).prop_flat_map(|n| {
            (addresses(n), proptest::collection::vec(0u32..6, n)).prop_map(
                |(program_ids, stack_depths)| {
                    program_ids
                        .into_iter()
                        .zip(stack_depths)
                        .map(|(program_id, stack_depth)| Parsed {
                            program_id,
                            operation: Operation::Unknown,
                            outer_idx: 0,
                            stack_depth,
                        })
                        .collect()
                },
            )
        })
    }

    /// Make sure that we never drop or reorder nodes, since we rely on the node indices to be the
    /// insertion order
    #[proptest]
    fn never_drops_or_reorders_nodes(
        #[strategy(arbitrary_instructions())] instructions: Vec<Parsed<Operation>>,
    ) {
        let expected_ids: Vec<Pubkey> = instructions.iter().map(|i| i.program_id).collect();
        let tree = CallTree::new(instructions);

        prop_assert_eq!(tree.tree().node_count(), expected_ids.len());
        for (i, program_id) in expected_ids.into_iter().enumerate() {
            let weight = tree.tree().node_weight(NodeIndex::new(i)).unwrap();
            prop_assert_eq!(weight.program_id, program_id);
        }
    }
}
