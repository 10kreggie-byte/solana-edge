import { createHash } from "node:crypto";
import * as splToken from "@solana/spl-token";
import {
  INITIALIZE_DISCRIMINATOR,
  PUBLISH_EPOCH_DISCRIMINATOR,
} from "./lib.mjs";

function discriminator(name) {
  return createHash("sha256")
    .update(`global:${name}`)
    .digest()
    .subarray(0, 8);
}

function assertEqual(actual, expected, label) {
  if (!Buffer.from(actual).equals(Buffer.from(expected))) {
    throw new Error(
      `${label} mismatch: expected ${Buffer.from(expected).toString("hex")}, got ${Buffer.from(actual).toString("hex")}`,
    );
  }
}

assertEqual(
  INITIALIZE_DISCRIMINATOR,
  discriminator("initialize"),
  "initialize discriminator",
);
assertEqual(
  PUBLISH_EPOCH_DISCRIMINATOR,
  discriminator("publish_epoch"),
  "publish_epoch discriminator",
);

if (typeof splToken.setAuthority !== "function") {
  throw new Error("@solana/spl-token setAuthority export is unavailable");
}
if (splToken.AuthorityType?.MintTokens !== 0) {
  throw new Error("@solana/spl-token AuthorityType.MintTokens is unexpected");
}

console.log("operator invariants: ok");
