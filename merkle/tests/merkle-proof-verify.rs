use merkle_generator::IterativeTrie;
use types::{EventProof, H256};

mod common;

fn merkle_proof_test(test_block: &str, test_block_receipts: &str) {
    let (hash, block_header) = common::load_block(test_block);
    let block_hash = H256::hash(&block_header);
    assert_eq!(hash, block_hash);

    let receipts = common::load_receipts(test_block_receipts);
    let mut trie = merkle_generator::PatriciaTrie::new();
    receipts.iter().enumerate().for_each(|(i, receipt)| {
        trie.insert(alloy_rlp::encode(i), alloy_rlp::encode(receipt));
    });

    for (i, receipt) in receipts.iter().enumerate() {
        let key = alloy_rlp::encode(i);
        let proof = trie.merkle_proof(key);
        let hash = H256::hash(receipt);
        let proof = EventProof {
            block_hash,
            block_header: block_header.clone(),
            transaction_receipt: receipt.clone(),
            transaction_receipt_hash: hash,
            merkle_proof_of_receipt: proof,
        };

        proof.validate().unwrap()
    }
}

#[test]
fn merkle_proof_17819525() {
    let test_block = include_str!("../tests/suits/block_17819525.json");
    let block_receipts = include_str!("../tests/suits/block_17819525_receipts.json");
    merkle_proof_test(test_block, block_receipts)
}

#[test]
fn merkle_proof_18027905() {
    let test_block = include_str!("../tests/suits/block_18027905.json");
    let block_receipts = include_str!("../tests/suits/block_18027905_receipts.json");
    merkle_proof_test(test_block, block_receipts)
}

#[test]
fn merkle_proof_8652100() {
    let test_block = include_str!("../tests/suits/block_8652100.json");
    let block_receipts = include_str!("../tests/suits/block_8652100_receipts.json");
    merkle_proof_test(test_block, block_receipts)
}
