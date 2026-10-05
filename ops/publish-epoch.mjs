import fs from "node:fs";
import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  PUBLISH_EPOCH_DISCRIMINATOR,
  hex32,
  loadKeypair,
  u64Le,
} from "./lib.mjs";

const manifestPath = process.argv[2];
if (!manifestPath) {
  throw new Error("usage: node publish-epoch.mjs <reward-manifest.json>");
}

const PROGRAM_ID = process.env.SOLANA_EDGE_REWARDS_PROGRAM_ID;
if (!PROGRAM_ID) {
  throw new Error("SOLANA_EDGE_REWARDS_PROGRAM_ID is required");
}

const RPC_URL = process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com";
const WALLET_PATH =
  process.env.SOLANA_WALLET ?? "~/.config/solana/id.json";

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const rewardEpoch = BigInt(manifest.reward_epoch);
const totalAmount = BigInt(manifest.total_amount);
const merkleRoot = hex32(manifest.merkle_root_hex, "merkle_root_hex");

if (totalAmount <= 0n) {
  throw new Error("reward manifest total_amount must be greater than zero");
}

const DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1";

const connection = new Connection(RPC_URL, "confirmed");
const genesisHash = await connection.getGenesisHash();
if (genesisHash !== DEVNET_GENESIS_HASH) {
  throw new Error(
    `Refusing epoch publication: expected Solana Devnet genesis ${DEVNET_GENESIS_HASH}, got ${genesisHash}`,
  );
}

const authority = loadKeypair(WALLET_PATH);
const programId = new PublicKey(PROGRAM_ID);
const epochBytes = u64Le(rewardEpoch);

const [configPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("config")],
  programId,
);
const [epochPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("epoch"), epochBytes],
  programId,
);

if (await connection.getAccountInfo(epochPda, "confirmed")) {
  throw new Error(
    `Reward epoch ${rewardEpoch} is already published at ${epochPda.toBase58()}`,
  );
}

const data = Buffer.concat([
  PUBLISH_EPOCH_DISCRIMINATOR,
  epochBytes,
  merkleRoot,
  u64Le(totalAmount),
]);

const transaction = new Transaction().add(
  new TransactionInstruction({
    programId,
    keys: [
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: authority.publicKey, isSigner: true, isWritable: true },
      { pubkey: epochPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  }),
);

const signature = await sendAndConfirmTransaction(
  connection,
  transaction,
  [authority],
  { commitment: "confirmed" },
);

console.log(
  JSON.stringify(
    {
      reward_epoch: rewardEpoch.toString(),
      merkle_root_hex: manifest.merkle_root_hex,
      total_amount: totalAmount.toString(),
      epoch_pda: epochPda.toBase58(),
      signature,
    },
    null,
    2,
  ),
);
