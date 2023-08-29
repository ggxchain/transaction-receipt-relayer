mod receipt;
pub use receipt::{
    Log, Receipt, ReceiptMerkleProof, ReceiptMerkleProofNode, TransactionReceipt, TxType,
};

mod primitives;
pub use primitives::{H160, H256, H64, U256};

mod block_header;
pub use block_header::BlockHeader;

mod bloom;
pub use bloom::Bloom;

pub(crate) mod encode;

pub struct EventProof {
    /// Block corresponding to a [stored block hash][1] in Webb's `pallet-eth2-light-client`.
    /// The hash of this structure is computed using its [rlp][2] representation. In particular, this is the 12th field of `execution_payload`,
    /// which is the 9th field of `body`. See the Ethereum documentation for [What's in a
    /// Block?][4].
    ///
    /// For a reference derivation of this field, see the [`reth` source code][3].
    ///
    /// [1]: https://github.com/webb-tools/pallet-eth2-light-client/blob/4d8a20ad325795a2d166fcd2a6118db3037581d3/pallet/src/lib.rs#L221-L233
    /// [2]: https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/
    /// [3]: https://ethereum.org/en/developers/docs/blocks/#block-anatomy
    /// [4]: https://github.com/paradigmxyz/reth/blob/15bb1c90b8e60dcaaaa1d2cbc82817d135192cbd/crates/rpc/rpc-types/src/eth/engine/payload.rs#L151-L178
    pub block: BlockHeader,

    /// Hash of the block.
    pub block_hash: H256,

    /// A transaction receipt. Must contain an event we are configured to listen to emitted by a
    /// configured smart contract address.
    pub transaction_receipt: TransactionReceipt,

    /// Hash of the transaction receipt.
    pub transaction_receipt_hash: H256,

    /// A Merkle proof that the transaction receipt has been included in the `receipt_root` field in
    /// the `block`.
    pub merkle_proof_of_receipt: ReceiptMerkleProof,

    pub transaction_index: u64,
}

/// Error type for validating `EventProofTransaction`s.
pub enum ValidationError {
    IncorrectBodyHash { expected: H256, actual: H256 },
    IncorrectReceiptHash { expected: H256, actual: H256 },
    IncorrectReceiptRoot { expected: H256, actual: H256 },
}

impl EventProof {
    /// Check that the `EventProofTransaction` is valid.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.block_hash != H256::hash(&self.block) {
            return Err(ValidationError::IncorrectBodyHash {
                expected: self.block_hash.clone(),
                actual: H256::hash(&self.block),
            });
        }
        if self.transaction_receipt_hash != H256::hash(&self.transaction_receipt) {
            return Err(ValidationError::IncorrectReceiptHash {
                expected: self.transaction_receipt_hash.clone(),
                actual: H256::hash(&self.transaction_receipt),
            });
        }
        if self.block.receipts_root
            != self
                .merkle_proof_of_receipt
                .merkle_root(&self.transaction_receipt, self.transaction_index)
        {
            return Err(ValidationError::IncorrectReceiptRoot {
                expected: self.block.receipts_root.clone(),
                actual: self
                    .merkle_proof_of_receipt
                    .merkle_root(&self.transaction_receipt, self.transaction_index),
            });
        }
        Ok(())
    }
}
