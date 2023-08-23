use alloy_rlp::{length_of_length, BufMut, Encodable};

use crate::{encode::rlp_node, H256};

#[derive(Debug)]
pub struct BranchNode {
    pub branches: [Option<H256>; 16],
}

impl Encodable for BranchNode {
    fn encode(&self, result: &mut dyn BufMut) {
        let mut buf = vec![];

        // 1 for the branch index + (32 for hash or 1 for empty string)
        let payload_length = 1 + self.branches.iter().fold(0, |acc, i| {
            if let Some(hash) = i {
                acc + hash.length()
            } else {
                acc + 1
            }
        });

        let header = alloy_rlp::Header {
            payload_length,
            list: true,
        };
        header.encode(&mut buf);

        for i in self.branches.iter() {
            if let Some(hash) = i {
                hash.encode(&mut buf);
            } else {
                buf.put_u8(alloy_rlp::EMPTY_STRING_CODE);
            }
        }

        buf.put_u8(alloy_rlp::EMPTY_STRING_CODE);
        rlp_node(&buf, result);
    }

    fn length(&self) -> usize {
        let mut length = 0;

        length += 1;

        for i in self.branches.iter() {
            if let Some(hash) = i {
                length += hash.length();
            } else {
                length += 1;
            }
        }

        length_of_length(length) + length
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use alloy_rlp::Encodable;
    use cita_trie::{MemoryDB, PatriciaTrie};
    use hasher::HasherKeccak;

    use crate::{
        receipt::trie::{leaf::ReceiptLeaf, nibble::Nibbles},
        Bloom, Log, Receipt, TransactionReceipt, H160, H256,
    };

    use super::BranchNode;

    #[test]
    fn full_branch_node_encoding() {
        // Test different branch node sizes
        for j in 1..16 {
            let mut branch_node = BranchNode {
                branches: Default::default(),
            };

            let mut cita_branch = cita_trie::node::BranchNode {
                children: cita_trie::node::empty_children(),
                value: None,
            };
            // Test branch with node filled up to j
            for i in 0..j {
                let receipt = TransactionReceipt {
                    bloom: Bloom::new([i; 256]),
                    receipt: Receipt {
                        cumulative_gas_used: i as u64,
                        logs: vec![Log {
                            address: H160([i; 20]),
                            topics: vec![H256([i; 32])],
                            data: vec![i],
                        }],
                        tx_type: crate::TxType::EIP1559,
                        success: true,
                    },
                };

                let mut receipt_encoded = vec![];
                receipt.encode(&mut receipt_encoded);

                let leaf = ReceiptLeaf::new(Nibbles::new(vec![i]), receipt);
                let mut buffer = vec![];
                leaf.encode(&mut buffer);
                branch_node.branches[i as usize] = Some(H256(buffer[..32].try_into().unwrap()));

                cita_branch.insert(
                    i as usize,
                    cita_trie::node::Node::Leaf(Rc::new(RefCell::new(cita_trie::node::LeafNode {
                        key: cita_trie::nibbles::Nibbles::from_raw(vec![i], true),
                        value: receipt_encoded,
                    }))),
                )
            }

            let mut encoded = vec![];
            branch_node.encode(&mut encoded);

            let trie =
                PatriciaTrie::new(Arc::new(MemoryDB::new(true)), Arc::new(HasherKeccak::new()));
            let cita_encoded = trie.encode_node(cita_trie::node::Node::Branch(Rc::new(
                RefCell::new(cita_branch),
            )));

            assert_eq!(encoded, cita_encoded);
        }
    }
}
