# Web Miner

Initial UI requirements:

- connect Solana wallet;
- show network = Devnet;
- choose CPU limit;
- choose local storage contribution;
- start/stop contribution explicitly;
- show jobs received/completed/verified;
- show pending test rewards;
- never run hidden background mining.

The browser will call the Rust miner through the WebAssembly wrapper in `miner/wasm`.
