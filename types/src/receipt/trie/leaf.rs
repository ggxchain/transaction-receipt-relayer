use alloc::vec::Vec;
use alloy_rlp::{BufMut, Encodable};

use crate::{encode, receipt::trie::nibble::Nibbles, TransactionReceipt};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Leaf {
    key: Vec<u8>,
    value: Vec<u8>,
}

impl Leaf {
    pub fn from_raw(key: Vec<u8>, value: Vec<u8>) -> Self {
        Self { key, value }
    }

    pub fn from_transaction_receipt(key: Nibbles, value: TransactionReceipt) -> Self {
        Self {
            key: key.encode_compact(),
            value: alloy_rlp::encode(value),
        }
    }
}

impl Encodable for Leaf {
    fn encode(&self, result: &mut dyn BufMut) {
        LeafEncoder {
            key: &self.key,
            value: &self.value,
        }
        .encode(result);
    }

    fn length(&self) -> usize {
        LeafEncoder {
            key: &self.key,
            value: &self.value,
        }
        .length()
    }
}

pub struct LeafEncoder<'a> {
    pub key: &'a [u8],
    pub value: &'a [u8],
}

impl<'a> LeafEncoder<'a> {
    fn header(&self) -> alloy_rlp::Header {
        alloy_rlp::Header {
            payload_length: self.key.length() + self.value.length(),
            list: true,
        }
    }
}

impl<'a> Encodable for LeafEncoder<'a> {
    fn encode(&self, result: &mut dyn BufMut) {
        let header = self.header();
        let mut out = Vec::with_capacity(header.payload_length);
        let out_buf = &mut out;
        encode!(out_buf, header, self.key, self.value);

        crate::encode::rlp_node(&out, result);
    }

    fn length(&self) -> usize {
        let header = self.header();
        alloy_rlp::length_of_length(header.payload_length) + header.payload_length
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use alloy_rlp::Encodable;
    use cita_trie::{node::LeafNode, MemoryDB, PatriciaTrie};
    use hasher::HasherKeccak;
    use test_strategy::proptest;

    use crate::{
        receipt::trie::{leaf::Leaf, nibble::Nibbles},
        Bloom, Log, Receipt, TransactionReceipt, H160, H256,
    };

    #[proptest]
    fn encode_leaf(data: Vec<u8>, number: u8, key: Vec<u8>) {
        let receipt = TransactionReceipt {
            bloom: Bloom::new([number; 256]),
            receipt: Receipt {
                cumulative_gas_used: number as u64,
                logs: vec![Log {
                    address: H160([number; 20]),
                    topics: vec![H256([number; 32])],
                    data,
                }],
                tx_type: crate::TxType::EIP1559,
                success: true,
            },
        };

        let mut receipt_encoded = vec![];
        receipt.encode(&mut receipt_encoded);

        let our_leaf =
            Leaf::from_transaction_receipt(Nibbles::from_raw(key.clone(), true), receipt);

        let mut our_leaf_encoded = vec![];
        our_leaf.encode(&mut our_leaf_encoded);

        let trie = PatriciaTrie::new(Arc::new(MemoryDB::new(true)), Arc::new(HasherKeccak::new()));

        let node = LeafNode {
            key: cita_trie::nibbles::Nibbles::from_raw(key, true),
            value: receipt_encoded,
        };
        let encoded = trie.encode_node(cita_trie::node::Node::Leaf(Rc::new(RefCell::new(node))));
        assert_eq!(&our_leaf_encoded, &encoded);
    }
}
