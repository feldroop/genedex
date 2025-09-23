// static binary search tree, heap-like memory layout in flat array
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[derive(Debug, Clone)]
pub(crate) struct TexdIdSearchTree {
    nodes: Vec<Node>,
    pub(crate) sentinel_indices: Vec<usize>,
}

impl TexdIdSearchTree {
    // indices assumed to be sorted
    pub(crate) fn new_from_sentinel_indices(sentinel_indices: Vec<usize>) -> Self {
        assert!(!sentinel_indices.is_empty());
        // this is required for the encoding of Node
        assert!(*sentinel_indices.last().unwrap() < isize::MAX as usize);

        let mut max_index_used = 0;

        let max_needed_values = sentinel_indices.len().next_power_of_two() * 2 - 1;

        let mut nodes = vec![Node::new_inner(0); max_needed_values];

        add_nodes(&mut nodes, 0, &sentinel_indices, 0, &mut max_index_used);

        nodes.truncate(max_index_used + 1);
        nodes.shrink_to_fit();

        Self {
            nodes,
            sentinel_indices,
        }
    }

    pub(crate) fn backtransfrom_concatenated_text_index(
        &self,
        concatenated_text_index: usize,
    ) -> (usize, usize) {
        let text_id = self.lookup_text_id(concatenated_text_index);

        let text_index = if text_id == 0 {
            concatenated_text_index
        } else {
            concatenated_text_index - self.sentinel_indices[text_id - 1] - 1
        };

        (text_id, text_index)
    }

    pub(crate) fn lookup_text_id(&self, concatenated_text_index: usize) -> usize {
        let mut curr_node_index = 0;

        while self.nodes[curr_node_index].is_inner() {
            curr_node_index = if concatenated_text_index
                <= self.nodes[curr_node_index].get_threshold_for_inner()
            {
                left_child_index(curr_node_index)
            } else {
                right_child_index(curr_node_index)
            };
        }

        self.nodes[curr_node_index].get_text_id_for_leaf()
    }
}

fn add_nodes(
    nodes: &mut [Node],
    curr_node_index: usize,
    indices: &[usize],
    indices_offset: usize,
    max_index_used: &mut usize,
) {
    *max_index_used = (*max_index_used).max(curr_node_index);

    let num_indices = indices.len();

    if num_indices == 1 {
        nodes[curr_node_index] = Node::new_leaf(indices_offset);
        return;
    }

    let curr_offset = if num_indices.is_power_of_two() {
        num_indices / 2
    } else {
        num_indices.next_power_of_two() / 2
    };

    let (left, right) = indices.split_at(curr_offset);
    let threshold = *left.last().unwrap();

    nodes[curr_node_index] = Node::new_inner(threshold);

    add_nodes(
        nodes,
        left_child_index(curr_node_index),
        left,
        indices_offset,
        max_index_used,
    );

    add_nodes(
        nodes,
        right_child_index(curr_node_index),
        right,
        indices_offset + curr_offset,
        max_index_used,
    );
}

fn left_child_index(curr_node_index: usize) -> usize {
    curr_node_index * 2 + 1
}

fn right_child_index(curr_node_index: usize) -> usize {
    (curr_node_index + 1) * 2
}

// this encodes the threshold as a positive value and a text id as its bit flipped (negative) value
// this is a space optimization meant to improve speed by making the structure as small and cache-friendly
// as possible
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[derive(Debug, Clone, Copy)]
struct Node {
    data: isize,
}

impl Node {
    fn new_inner(threshold: usize) -> Self {
        Self {
            data: threshold as isize,
        }
    }

    fn is_inner(&self) -> bool {
        self.data >= 0
    }

    fn get_threshold_for_inner(&self) -> usize {
        self.data as usize
    }

    fn new_leaf(text_id: usize) -> Self {
        Self {
            data: (!text_id as isize),
        }
    }

    fn get_text_id_for_leaf(&self) -> usize {
        (!self.data) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_text_lookup() {
        let sentinel_indices = vec![10, 21, 32, 50, 68, 140, 141];
        let text_ids = TexdIdSearchTree::new_from_sentinel_indices(sentinel_indices);

        assert_eq!(0, text_ids.lookup_text_id(5));
        assert_eq!(1, text_ids.lookup_text_id(21));
        assert_eq!(0, text_ids.lookup_text_id(0));
        assert_eq!(5, text_ids.lookup_text_id(140));
        assert_eq!(6, text_ids.lookup_text_id(141));
        assert_eq!(3, text_ids.lookup_text_id(33));
        assert_eq!(4, text_ids.lookup_text_id(67));
    }
}
