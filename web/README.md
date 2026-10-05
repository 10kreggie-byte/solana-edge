# Web Miner

The browser miner is a Vite/React prototype using Solana Wallet Adapter with Wallet Standard discovery and the shared Rust miner compiled to WebAssembly.

## Local run

From the repository root:

```bash
eval "$(bash scripts/generate-dev-receipt-key.sh)"
cargo run -p solana-edge-coordinator --bin server
```

Then:

```bash
cd web
npm install
npm run dev
```

The default coordinator URL is `http://127.0.0.1:8787`. Copy `.env.example` to `.env.local` to override it.

## Worker flow

1. Connect a Solana wallet.
2. Press **Start contributing**.
3. Sign one human-readable worker-authentication message.
4. Receive a short-lived session token.
5. Fetch a finalized Devnet job.
6. Execute it locally through Rust/WASM.
7. Submit the deterministic result.
8. Receive a signed WorkReceipt only after independent verification.

The miner stops when the user presses Stop or when the page leaves the foreground. No transaction is requested during worker authentication.
