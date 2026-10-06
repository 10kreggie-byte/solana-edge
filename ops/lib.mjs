import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { Keypair } from "@solana/web3.js";

export const INITIALIZE_DISCRIMINATOR = Buffer.from([
  175, 175, 109, 31, 13, 152, 155, 237,
]);

export const PUBLISH_EPOCH_DISCRIMINATOR = Buffer.from([
  222, 6, 136, 82, 54, 246, 245, 120,
]);

export function expandHome(value) {
  if (value === "~") return os.homedir();
  if (value.startsWith("~/")) return path.join(os.homedir(), value.slice(2));
  return value;
}

export function loadKeypair(filePath) {
  const resolved = expandHome(filePath);
  const secret = JSON.parse(fs.readFileSync(resolved, "utf8"));
  if (!Array.isArray(secret)) {
    throw new Error(`Keypair file must contain a JSON byte array: ${resolved}`);
  }
  return Keypair.fromSecretKey(Uint8Array.from(secret));
}

export function u64Le(value) {
  const bigint = BigInt(value);
  if (bigint < 0n || bigint > 0xffff_ffff_ffff_ffffn) {
    throw new Error("u64 value is out of range");
  }

  const bytes = Buffer.alloc(8);
  let remaining = bigint;
  for (let index = 0; index < 8; index += 1) {
    bytes[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return bytes;
}

export function hex32(value, fieldName) {
  const bytes = Buffer.from(value, "hex");
  if (bytes.length !== 32) {
    throw new Error(`${fieldName} must decode to exactly 32 bytes`);
  }
  return bytes;
}
