use parser_core::instruction::ParsedInstruction;
use petgraph::graph::{DiGraph, NodeIndex};

pub struct CallTree {
    tree: DiGraph<ParsedInstruction, ()>,
    roots: Vec<NodeIndex>,
}

impl CallTree {
    /**
     * Builds the literal instruction call tree for a transaction, from
     * every [ParsedInstruction] including [parser_core::instruction::operation::Operation::Unknown]
     * placeholders. One root per top-level instruction.
     */
    pub fn new(instructions: Vec<ParsedInstruction>) -> CallTree {
        let mut tree = DiGraph::new();
        let mut roots = Vec::new();
        let mut stack: Vec<(u32, NodeIndex)> = Vec::new();

        for instr in instructions {
            let node = tree.add_node(instr);
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

    pub fn tree(&self) -> &DiGraph<ParsedInstruction, ()> {
        &self.tree
    }
}
