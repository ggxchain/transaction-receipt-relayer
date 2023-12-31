use alloc::vec::Vec;
use alloy_rlp::{length_of_length, BufMut, Encodable};

use crate::{encode::rlp_node, H256};

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BranchNode {
    pub branches: [Option<H256>; 16],
    pub value: Option<Vec<u8>>,
}

impl BranchNode {
    fn header(&self) -> alloy_rlp::Header {
        // 1 for the branch index + (32 for hash or 1 for empty string)
        let payload_length = 1 + self.branches.iter().fold(0, |acc, i| {
            if let Some(hash) = i {
                acc + hash.length()
            } else {
                acc + 1
            }
        });
        alloy_rlp::Header {
            list: true,
            payload_length,
        }
    }
}

impl Encodable for BranchNode {
    fn encode(&self, result: &mut dyn BufMut) {
        let header = self.header();
        let mut buf = Vec::with_capacity(header.payload_length);
        let buf_mut = &mut buf;
        crate::encode!(buf_mut, header);
        for i in self.branches.iter() {
            if let Some(hash) = i {
                crate::encode!(buf_mut, hash);
            } else {
                buf_mut.put_u8(alloy_rlp::EMPTY_STRING_CODE);
            }
        }

        if let Some(value) = &self.value {
            buf_mut.put_slice(value);
        } else {
            buf_mut.put_u8(alloy_rlp::EMPTY_STRING_CODE);
        }
        rlp_node(&buf, result);
    }

    fn length(&self) -> usize {
        let length = self.header().payload_length;

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
        receipt::trie::{leaf::Leaf, nibble::Nibbles},
        Bloom, Log, Receipt, TransactionReceipt, H160, H256,
    };

    use super::BranchNode;

    #[test]
    fn full_branch_node_encoding() {
        // Test different branch node sizes
        for j in 1..16 {
            let mut branch_node = BranchNode {
                branches: Default::default(),
                value: None,
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

                let leaf =
                    Leaf::from_transaction_receipt(Nibbles::from_raw(vec![i], true), receipt);
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
