import fs from "node:fs";
import path from "node:path";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createMint,
  getAssociatedTokenAddressSync,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  INITIALIZE_DISCRIMINATOR,
  loadKeypair,
} from "./lib.mjs";

const RPC_URL = process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com";
const WALLET_PATH =
  process.env.SOLANA_WALLET ?? "~/.config/solana/id.json";
const PROGRAM_KEYPAIR =
  process.env.SOLANA_EDGE_PROGRAM_KEYPAIR ??
  "target/deploy/solana_edge_rewards-keypair.json";
const INITIAL_SUPPLY = BigInt(
  process.env.SOLANA_EDGE_TEST_SUPPLY ?? "10000000",
);

const connection = new Connection(RPC_URL, "confirmed");
const payer = loadKeypair(WALLET_PATH);
const programKeypair = loadKeypair(PROGRAM_KEYPAIR);
const programId = programKeypair.publicKey;

const programAccount = await connection.getAccountInfo(programId, "confirmed");
if (!programAccount?.executable) {
  throw new Error(
    `Reward program ${programId.toBase58()} is not deployed/executable on Devnet.`,
  );
}

const [configPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("config")],
  programId,
);

const existingConfig = await connection.getAccountInfo(configPda, "confirmed");
if (existingConfig) {
  throw new Error(
    `Config PDA ${configPda.toBase58()} already exists. Refusing to create a second test mint.`,
  );
}

const mint = await createMint(
  connection,
  payer,
  payer.publicKey,
  null,
  0,
  undefined,
  { commitment: "confirmed" },
  TOKEN_PROGRAM_ID,
);

const vault = getAssociatedTokenAddressSync(
  mint,
  configPda,
  true,
  TOKEN_PROGRAM_ID,
);

const initializeTransaction = new Transaction().add(
  createAssociatedTokenAccountIdempotentInstruction(
    payer.publicKey,
    vault,
    configPda,
    mint,
    TOKEN_PROGRAM_ID,
  ),
  new TransactionInstruction({
    programId,
    keys: [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: vault, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: INITIALIZE_DISCRIMINATOR,
  }),
);

const initializeSignature = await sendAndConfirmTransaction(
  connection,
  initializeTransaction,
  [payer],
  { commitment: "confirmed" },
);

const mintSignature = await mintTo(
  connection,
  payer,
  mint,
  vault,
  payer,
  INITIAL_SUPPLY,
  [],
  { commitment: "confirmed" },
  TOKEN_PROGRAM_ID,
);

const deployment = {
  cluster: "devnet",
  rpc_url: RPC_URL,
  program_id: programId.toBase58(),
  authority: payer.publicKey.toBase58(),
  config_pda: configPda.toBase58(),
  reward_mint: mint.toBase58(),
  reward_vault: vault.toBase58(),
  test_supply_raw: INITIAL_SUPPLY.toString(),
  token_decimals: 0,
  initialize_signature: initializeSignature,
  mint_signature: mintSignature,
};

const outputPath =
  process.env.SOLANA_EDGE_DEPLOYMENT_OUTPUT ?? "devnet-deployment.json";
fs.writeFileSync(path.resolve(outputPath), JSON.stringify(deployment, null, 2) + "\n");

console.log(JSON.stringify(deployment, null, 2));
console.error("");
console.error("Coordinator environment:");
console.error(`export SOLANA_EDGE_REWARDS_PROGRAM_ID=${deployment.program_id}`);
console.error(`export SOLANA_EDGE_REWARD_MINT=${deployment.reward_mint}`);
