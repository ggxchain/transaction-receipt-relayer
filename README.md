# transaction-receipt-relayer

This repository contains an Ethereum transaction receipt relayer for the GGX network.
The relayer verifies finalized blocks using [Helios light client](https://github.com/a16z/helios).
The relayer requires a Beacon and Consensus RPC node, which updates and blocks will be verified.
It will relay transaction receipts for the smart contracts list managed by the pallet in this repository.

## Install dependencies

```bash
sudo apt install libsqlite3-dev
```

## Run

```bash
RUST_LOG=info cargo run --release -- --network sepolia --database db --helios-config-path helios.toml --substrate-config-path ggxchain-config.toml
```

### Configs

* GGX config
|| Field || Definition ||
|---|---|
|is_dev| if set to true the Alice account will be used, and the phrase will be ignored|
|ws_url| GGX RPC endpoint|
|phrase| Account for signing transaction.|
* [Helios config](https://github.com/a16z/helios/blob/master/config.md)

Please note that you need to update helios.toml checkpoint from time to time.

### Action points to look

* Check how it works if multiple relayers are working simultaneously.
* Optimize batch sending of receipts.