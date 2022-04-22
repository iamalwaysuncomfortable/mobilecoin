// Copyright (c) 2018-2022 The MobileCoin Foundation

use crate::tx_manager::TxManagerResult;
use mc_attest_enclave_api::{EnclaveMessage, PeerSession};
use mc_common::HashSet;
use mc_consensus_enclave::{TxContext, WellFormedEncryptedTx};
use mc_transaction_core::tx::{TxHash, TxOutMembershipProof};

#[cfg(test)]
use mockall::*;

#[cfg_attr(test, automock)]
pub trait TxManager: Send {
    /// Insert a transaction into the cache. The transaction must be
    /// well-formed.
    fn insert(&self, tx_context: TxContext) -> TxManagerResult<TxHash>;

    /// Remove expired transactions from the cache and return their hashes.
    ///
    /// # Arguments
    /// * `block_index` - Current block index.
    fn remove_expired(&self, block_index: u64) -> HashSet<TxHash>;

    /// Returns true if the cache contains the corresponding transaction.
    fn contains(&self, tx_hash: &TxHash) -> bool;

    /// Number of cached entries.
    fn num_entries(&self) -> usize;

    /// Validate the transaction corresponding to the given hash against the
    /// current ledger.
    fn validate(&self, tx_hash: &TxHash) -> TxManagerResult<()>;

    /// Combines the transactions that correspond to the given hashes.
    fn combine(&self, tx_hashes: &[TxHash]) -> TxManagerResult<Vec<TxHash>>;

    /// Get an array of well-formed encrypted transactions and membership proofs
    /// that correspond to the provided tx hashes.
    ///
    /// # Arguments
    /// * `tx_hashes` - Hashes of well-formed transactions that are valid w.r.t.
    ///   the current ledger.
    fn tx_hashes_to_well_formed_encrypted_txs_and_proofs(
        &self,
        value: &[TxHash],
    ) -> TxManagerResult<Vec<(WellFormedEncryptedTx, Vec<TxOutMembershipProof>)>>;

    /// Creates a message containing a set of transactions that are encrypted
    /// for a peer.
    ///
    /// # Arguments
    /// * `tx_hashes` - transaction hashes.
    /// * `aad` - Additional authenticated data.
    /// * `peer` - Recipient of the encrypted message.
    fn encrypt_for_peer(
        &self,
        tx_hashes: &[TxHash],
        aad: &[u8],
        peer: &PeerSession,
    ) -> TxManagerResult<EnclaveMessage<PeerSession>>;

    /// Get the encrypted transaction corresponding to the given hash.
    fn get_encrypted_tx(&self, tx_hash: &TxHash) -> Option<WellFormedEncryptedTx>;
}
