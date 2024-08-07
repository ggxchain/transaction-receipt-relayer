use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use ethers::providers::{Http, Provider};
use ethers::{providers::Middleware, types::U64};
use eyre::Result;
use helios::{
    client::{Client as HeliosClient, ClientBuilder, FileDB},
    types::{Block, BlockTag},
};
use types::{BlockHeaderWithTransaction, H160, H256};

use crate::{
    common::*,
    config::Config,
    consts::{BLOCK_AMOUNT_TO_STORE, SLEEP_DURATION},
    db::DB,
    network_name_to_id,
    substrate_client::SubstrateClient,
};

pub struct Client {
    client: HeliosClient<FileDB>,
    block_rpc: Provider<Http>,
    db: DB,
    term: Arc<AtomicBool>,
    substrate_client: SubstrateClient,
    chain_id: u32,
    // TODO: rotate blocks in the database
    blocks_to_store: u64,

    // Cache of watched addresses
    watched_addresses: Option<Vec<H160>>,
}

impl Client {
    pub fn new(
        config: Config,
        db: DB,
        term: Arc<AtomicBool>,
        substrate_client: SubstrateClient,
    ) -> Result<Self> {
        let helios_config = prepare_config(&config);
        let block_rpc =
            Provider::<Http>::try_from(&helios_config.execution_rpc).map_err(|err| {
                eyre::eyre!(
                    "Failed to connect to Ethereum RPC at {} with error: {}",
                    helios_config.execution_rpc,
                    err
                )
            })?;
        let client: HeliosClient<FileDB> = ClientBuilder::new()
            .config(helios_config)
            .data_dir(config.database.join("helios"))
            .build()?;
        Ok(Client {
            client,
            block_rpc,
            db,
            term,
            substrate_client,
            chain_id: network_name_to_id(&config.network)?,
            blocks_to_store: config.blocks_to_store.unwrap_or(BLOCK_AMOUNT_TO_STORE),
            watched_addresses: None,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        exit_if_term(self.term.clone());
        log::info!(target: "relayer::client::start","starting client");
        self.client.start().await?;
        log::info!(target: "relayer::client::start","client started");

        self.finalization_loop().await?;

        Ok(())
    }

    /// Tries to get finalized block from Helios and start fetching if any updates are available.
    async fn finalization_loop(&mut self) -> Result<()> {
        const TARGET: &str = "relayer::client::finalization_loop";

        let mut latest_fetched_block = self.db.select_latest_fetched_block_height()?;
        loop {
            exit_if_term(self.term.clone());
            tokio::time::sleep(SLEEP_DURATION).await;
            let finalized_block = self
                .client
                .get_block_by_number(BlockTag::Finalized, false)
                .await;
            let finalized_block = if let Ok(Some(finalized_block)) = finalized_block {
                finalized_block
            } else {
                log::warn!(target: TARGET,"Failed to get finalized block, retrying in {} seconds", SLEEP_DURATION.as_secs());
                continue;
            };

            if Some(finalized_block.number.as_u64()) == latest_fetched_block {
                log::info!(target: TARGET,"No new finalized blocks, retrying in {} seconds", SLEEP_DURATION.as_secs());
                continue;
            }
            log::info!(target: TARGET,"New blocks to fetch. Latest finalized: {}, Latest processed: {latest_fetched_block:?}", finalized_block.number );

            // We have received finality update. It happens not that often, let's check watched addresses.
            if let Ok(watched_addresses) =
                self.substrate_client.watched_addresses(self.chain_id).await
            {
                // Update cache only if we have successfully fetched
                // TODO: ideally after we noticed that we have new addresses, we should check for blocks stored in db to verify that we didn't miss some txs
                self.watched_addresses = Some(watched_addresses);
            }

            // If we could never get watched addresses, there is no point in fetching blocks.
            if self.watched_addresses.is_none() {
                log::warn!(target: TARGET,"Failed to get watched addresses, retrying in {} seconds", SLEEP_DURATION.as_secs());
                continue;
            }

            if let Err(e) = self
                .collect_blocks_after_finality_update(finalized_block, latest_fetched_block)
                .await
            {
                log::error!(target: TARGET,"Failed to process finality update: {}", e);
            } else {
                log::info!(target: TARGET,"Processed finality update");
            };

            // Update latest fetched block after fetching. This is needed to avoid querying db on every iteration.
            latest_fetched_block = self.db.select_latest_fetched_block_height()?;
        }
    }

    /// Fetches all blocks from the web3 provider. The fetching goes backwards from the latest finalized block
    /// to the latest processed block using parent hash.
    async fn collect_blocks_after_finality_update(
        &mut self,
        finalized_block: Block,
        latest_fetched_block: Option<u64>,
    ) -> Result<()> {
        const TARGET: &str = "relayer::client::collect_blocks_after_finality_update";

        log::info!(target: TARGET,"Processing finality update");
        let latest_fetched_block =
            latest_fetched_block.unwrap_or(finalized_block.number.as_u64() - self.blocks_to_store);

        log::info!(target: TARGET,"Latest fetched block: {}", latest_fetched_block);

        // Now we have fetch missing blocks using previous block hash until we hit latest processed block.
        // If it's first run, we have to backtrack for self.block_to_fetch blocks.
        let mut blocks_to_process =
            Vec::with_capacity((finalized_block.number.as_u64() - latest_fetched_block) as usize);

        let mut current_block = finalized_block.number - 1;
        let mut prev_block_hash = finalized_block.parent_hash;
        let block = self
            .block_rpc
            .get_block(finalized_block.hash)
            .await?
            .ok_or_else(|| eyre::eyre!("Block not found"))?;
        // push first finalized block to the queue
        blocks_to_process.push((convert_ethers_block(block)?, H256(finalized_block.hash.0)));

        let mut repeat = 0;

        while current_block.as_u64() != latest_fetched_block {
            // Fetch block by parent hash using web3 interface
            let execution_block = self.block_rpc.get_block(prev_block_hash).await;
            let execution_block = if let Ok(Some(execution_block)) = execution_block {
                execution_block
            } else {
                log::warn!(target: TARGET, "Failed to get block by hash.\nBlock number: {current_block}");
                repeat = repeat_cycle(repeat).await?;
                continue;
            };
            let tmp = execution_block.parent_hash;
            // parse block to our format
            if let Ok(parsed_block) = convert_ethers_block(execution_block) {
                // store requested hash to verify later
                blocks_to_process.push((parsed_block, H256(prev_block_hash.0)));
                current_block = current_block.saturating_sub(U64::one());
                prev_block_hash = tmp;
                // reset repeat as we had a success.
                repeat = 0;
            } else {
                log::warn!(target: TARGET, "Failed to parse block.\nBlock number: {current_block}");
                repeat = repeat_cycle(repeat).await?;
            }
        }
        self.process_fetched_blocks(blocks_to_process)?;

        Ok(())
    }

    /// Process fetched blocks, check the block hash, bloom filter and store records in the database.
    /// The blocks are processed from the latest processed block + 1 to the latest block.
    fn process_fetched_blocks(
        &mut self,
        blocks: Vec<(BlockHeaderWithTransaction, H256)>,
    ) -> Result<()> {
        const TARGET: &str = "relayer::client::process_fetched_blocks";

        let watched_addresses = self
            .watched_addresses
            .as_ref()
            .expect("This function should be called only after we have fetched watched addresses");

        if blocks.is_empty() {
            return Ok(());
        }

        // Load latest processed block hash from the database.
        let mut processed_block_hash = self
            .db
            .select_latest_fetched_block_hash()?
            .unwrap_or_else(|| blocks.last().unwrap().0.header.parent_hash);
        for (block, block_hash) in blocks.into_iter().rev() {
            // First initial check that it's in order. And that the parent block hash is expected.
            if processed_block_hash != block.header.parent_hash {
                log::error!(target: TARGET, "Block parent hash mismatch");
                return Err(eyre::eyre!("Block parent hash mismatch"));
            }

            // Verify block hash correctness
            let hash = H256::hash(&block.header);
            if hash != block_hash {
                log::error!(target: TARGET,"Block hash mismatch");
                return Err(eyre::eyre!("Block hash mismatch"));
            }

            let block_number = block.header.number;

            // Check the bloom filter over expected contracts
            let should_process = watched_addresses
                .iter()
                .any(|address| block.header.logs_bloom.check_address(address));

            // Store block in the database
            self.db
                .insert_block(block_number, block_hash, block, should_process)?;

            processed_block_hash = hash;
        }
        Ok(())
    }
}

async fn repeat_cycle(repeat_counter: u64) -> Result<u64> {
    const RETRIES: u64 = 10;
    if repeat_counter < RETRIES {
        log::warn!(target: "relayer::client::repeat_cycle","Sleeping for 5 seconds");
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(repeat_counter + 1)
    } else {
        log::error!(target: "relayer::client::repeat_cycle","Multiple retries happened. Exiting.");
        Err(eyre::eyre!("Multiple retries happened"))
    }
}
