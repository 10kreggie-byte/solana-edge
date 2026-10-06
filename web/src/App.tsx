import { useEffect, useRef, useState } from "react";
import { Buffer } from "buffer";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { useConnection, useWallet } from "@solana/wallet-adapter-react";
import { WalletMultiButton } from "@solana/wallet-adapter-react-ui";
import {
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import initWasm, { execute_job_json } from "./wasm-pkg/solana_edge_miner.js";

type JobSpec = {
  version: number;
  job_id: string;
  source_cluster: string;
  source_slot: number;
  source_blockhash: string;
  signatures: string[];
};

type JobResult = {
  job_id: string;
  source_slot: number;
  item_count: number;
  commitment_hex: string;
};

type WalletChallenge = {
  challenge_id: string;
  worker_pubkey: string;
  message: string;
  expires_at_unix: number;
};

type WorkerSession = {
  session_token: string;
  worker_pubkey: string;
  expires_at_unix: number;
};

type SignedWorkReceipt = {
  receipt: {
    receipt_id: string;
    worker_pubkey: string;
    job_id: string;
    source_slot: number;
    commitment_hex: string;
    item_count: number;
    score: number;
    reward_epoch: number;
    issued_at_unix: number;
  };
  signer_pubkey: string;
  signature_hex: string;
};

type CoordinatorConfig = {
  protocol_version: number;
  cluster: string;
  receipt_signer_pubkey: string;
  session_ttl_seconds: number;
  rewards_program_id: string | null;
  reward_mint: string | null;
};

type RewardProofNode = {
  hash_hex: string;
  sibling_is_left: boolean;
};

type RewardClaimResponse = {
  reward_epoch: string;
  merkle_root_hex: string;
  total_amount: string;
  claim: {
    worker_pubkey: string;
    amount: string;
    proof: RewardProofNode[];
  };
};

type Mode = "eco" | "balanced" | "max";

const API_URL = (
  import.meta.env.VITE_COORDINATOR_URL ?? "http://127.0.0.1:8787"
).replace(/\/$/, "");

const MODE_DELAY_MS: Record<Mode, number> = {
  eco: 15_000,
  balanced: 5_000,
  max: 1_500,
};

const CLAIM_DISCRIMINATOR = Uint8Array.from([
  62, 198, 214, 193, 213, 159, 108, 210,
]);

const textEncoder = new TextEncoder();

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function hexToBytes(value: string): Uint8Array {
  if (value.length % 2 !== 0) {
    throw new Error("Invalid hex length.");
  }

  const output = new Uint8Array(value.length / 2);
  for (let index = 0; index < output.length; index += 1) {
    output[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  return output;
}

function u64Le(value: bigint): Uint8Array {
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new Error("u64 value is out of range.");
  }

  const bytes = new Uint8Array(8);
  let remaining = value;
  for (let index = 0; index < 8; index += 1) {
    bytes[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return bytes;
}

function u32Le(value: number): Uint8Array {
  const bytes = new Uint8Array(4);
  new DataView(bytes.buffer).setUint32(0, value, true);
  return bytes;
}

function concatBytes(parts: Uint8Array[]): Uint8Array {
  const length = parts.reduce((total, part) => total + part.length, 0);
  const output = new Uint8Array(length);
  let offset = 0;

  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }

  return output;
}

function encodeClaimData(
  rewardEpoch: bigint,
  amount: bigint,
  proof: RewardProofNode[],
): Buffer {
  const nodes = proof.map((node) => {
    const hash = hexToBytes(node.hash_hex);
    if (hash.length !== 32) {
      throw new Error("Merkle proof node must be 32 bytes.");
    }
    return concatBytes([
      hash,
      Uint8Array.from([node.sibling_is_left ? 1 : 0]),
    ]);
  });

  return Buffer.from(
    concatBytes([
      CLAIM_DISCRIMINATOR,
      u64Le(rewardEpoch),
      u64Le(amount),
      u32Le(proof.length),
      ...nodes,
    ]),
  );
}

async function api<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`${API_URL}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(options?.headers ?? {}),
    },
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `HTTP ${response.status}`);
  }

  return response.json() as Promise<T>;
}

function short(value: string, size = 6): string {
  if (value.length <= size * 2 + 3) return value;
  return `${value.slice(0, size)}...${value.slice(-size)}`;
}

export default function App() {
  const { connection } = useConnection();
  const { publicKey, signMessage, connected, sendTransaction } = useWallet();
  const [wasmReady, setWasmReady] = useState(false);
  const [running, setRunning] = useState(false);
  const [mode, setMode] = useState<Mode>("balanced");
  const [status, setStatus] = useState("Initializing WebAssembly...");
  const [jobsCompleted, setJobsCompleted] = useState(0);
  const [verifiedJobs, setVerifiedJobs] = useState(0);
  const [score, setScore] = useState(0);
  const [lastReceipt, setLastReceipt] = useState<SignedWorkReceipt | null>(null);
  const [config, setConfig] = useState<CoordinatorConfig | null>(null);
  const [claimEpoch, setClaimEpoch] = useState("");
  const [claimQuote, setClaimQuote] = useState<RewardClaimResponse | null>(null);
  const [claimStatus, setClaimStatus] = useState("No reward proof loaded.");
  const [claiming, setClaiming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runningRef = useRef(false);
  const sessionRef = useRef<WorkerSession | null>(null);

  useEffect(() => {
    let cancelled = false;

    initWasm()
      .then(() => {
        if (!cancelled) {
          setWasmReady(true);
          setStatus("Ready");
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
          setStatus("WASM initialization failed");
        }
      });

    api<CoordinatorConfig>("/v1/config")
      .then((nextConfig) => {
        if (!cancelled) setConfig(nextConfig);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const handleVisibility = () => {
      if (document.hidden && runningRef.current) {
        runningRef.current = false;
        setRunning(false);
        setStatus("Paused because the tab left the foreground");
      }
    };

    document.addEventListener("visibilitychange", handleVisibility);
    return () => document.removeEventListener("visibilitychange", handleVisibility);
  }, []);

  useEffect(() => {
    sessionRef.current = null;
    setClaimQuote(null);
    setClaimStatus("No reward proof loaded.");
  }, [publicKey?.toBase58()]);

  useEffect(() => {
    if (lastReceipt) {
      setClaimEpoch(String(lastReceipt.receipt.reward_epoch));
    }
  }, [lastReceipt]);

  async function getSession(): Promise<WorkerSession> {
    const workerPubkey = publicKey?.toBase58();
    if (!workerPubkey) {
      throw new Error("Connect a Solana wallet first.");
    }
    if (!signMessage) {
      throw new Error("This wallet does not support message signing.");
    }

    const existing = sessionRef.current;
    const now = Math.floor(Date.now() / 1000);
    if (
      existing &&
      existing.worker_pubkey === workerPubkey &&
      existing.expires_at_unix > now + 15
    ) {
      return existing;
    }

    setStatus("Requesting wallet authentication...");
    const challenge = await api<WalletChallenge>("/v1/challenge", {
      method: "POST",
      body: JSON.stringify({ worker_pubkey: workerPubkey }),
    });

    const signature = await signMessage(new TextEncoder().encode(challenge.message));

    const session = await api<WorkerSession>("/v1/session", {
      method: "POST",
      body: JSON.stringify({
        challenge_id: challenge.challenge_id,
        worker_pubkey: workerPubkey,
        signature_hex: toHex(signature),
      }),
    });

    sessionRef.current = session;
    return session;
  }

  async function mineOne(session: WorkerSession): Promise<void> {
    setStatus("Fetching finalized Devnet job...");
    const job = await api<JobSpec>("/v1/job");

    setStatus(`Computing slot ${job.source_slot} in WASM...`);
    const result = JSON.parse(execute_job_json(JSON.stringify(job))) as JobResult;
    setJobsCompleted((count) => count + 1);

    setStatus("Submitting for independent verification...");
    const receipt = await api<SignedWorkReceipt>("/v1/submit", {
      method: "POST",
      body: JSON.stringify({
        session_token: session.session_token,
        worker_pubkey: session.worker_pubkey,
        job,
        result,
      }),
    });

    setVerifiedJobs((count) => count + 1);
    setScore((value) => value + receipt.receipt.score);
    setLastReceipt(receipt);
    setStatus(`Verified slot ${receipt.receipt.source_slot}`);
  }

  async function start(): Promise<void> {
    if (runningRef.current) return;
    if (!wasmReady) {
      setError("WebAssembly is not ready yet.");
      return;
    }
    if (!connected || !publicKey) {
      setError("Connect a Solana wallet first.");
      return;
    }

    setError(null);
    runningRef.current = true;
    setRunning(true);

    try {
      const session = await getSession();

      while (runningRef.current) {
        await mineOne(session);

        if (!runningRef.current) break;
        await new Promise((resolve) => setTimeout(resolve, MODE_DELAY_MS[mode]));
      }
    } catch (cause: unknown) {
      runningRef.current = false;
      setError(cause instanceof Error ? cause.message : String(cause));
      setStatus("Stopped");
    } finally {
      setRunning(false);
    }
  }

  function stop(): void {
    runningRef.current = false;
    setRunning(false);
    setStatus("Stopped by user");
  }

  async function loadReward(): Promise<void> {
    if (!publicKey) {
      setError("Connect a Solana wallet first.");
      return;
    }
    if (!claimEpoch.trim()) {
      setError("Enter a reward epoch.");
      return;
    }

    setError(null);
    setClaimQuote(null);
    setClaimStatus("Loading published reward proof...");

    try {
      const epoch = BigInt(claimEpoch.trim());
      if (epoch < 0n) throw new Error("Reward epoch must be positive.");

      const quote = await api<RewardClaimResponse>(
        `/v1/rewards/${epoch.toString()}/${publicKey.toBase58()}`,
      );

      if (quote.claim.worker_pubkey !== publicKey.toBase58()) {
        throw new Error("Coordinator returned a proof for a different wallet.");
      }

      setClaimQuote(quote);
      setClaimStatus(
        `Proof loaded: ${quote.claim.amount} Devnet test reward units.`,
      );
    } catch (cause: unknown) {
      setClaimStatus("No claim loaded.");
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function claimReward(): Promise<void> {
    if (!publicKey || !claimQuote) {
      setError("Load a reward proof first.");
      return;
    }
    if (!config?.rewards_program_id || !config.reward_mint) {
      setError("Reward program is not deployed/configured yet.");
      return;
    }

    setError(null);
    setClaiming(true);
    setClaimStatus("Building Devnet claim transaction...");

    try {
      const programId = new PublicKey(config.rewards_program_id);
      const rewardMint = new PublicKey(config.reward_mint);
      const rewardEpoch = BigInt(claimQuote.reward_epoch);
      const amount = BigInt(claimQuote.claim.amount);
      const epochBytes = u64Le(rewardEpoch);

      const [configPda] = PublicKey.findProgramAddressSync(
        [textEncoder.encode("config")],
        programId,
      );
      const [epochPda] = PublicKey.findProgramAddressSync(
        [textEncoder.encode("epoch"), epochBytes],
        programId,
      );
      const [claimPda] = PublicKey.findProgramAddressSync(
        [textEncoder.encode("claim"), epochBytes, publicKey.toBytes()],
        programId,
      );

      const vault = getAssociatedTokenAddressSync(
        rewardMint,
        configPda,
        true,
      );
      const workerTokenAccount = getAssociatedTokenAddressSync(
        rewardMint,
        publicKey,
      );

      if (await connection.getAccountInfo(claimPda, "confirmed")) {
        throw new Error("This wallet already claimed this reward epoch.");
      }

      const transaction = new Transaction();

      if (!(await connection.getAccountInfo(workerTokenAccount, "confirmed"))) {
        transaction.add(
          createAssociatedTokenAccountIdempotentInstruction(
            publicKey,
            workerTokenAccount,
            publicKey,
            rewardMint,
          ),
        );
      }

      transaction.add(
        new TransactionInstruction({
          programId,
          keys: [
            { pubkey: configPda, isSigner: false, isWritable: false },
            { pubkey: epochPda, isSigner: false, isWritable: true },
            { pubkey: claimPda, isSigner: false, isWritable: true },
            { pubkey: publicKey, isSigner: true, isWritable: true },
            { pubkey: rewardMint, isSigner: false, isWritable: false },
            { pubkey: vault, isSigner: false, isWritable: true },
            { pubkey: workerTokenAccount, isSigner: false, isWritable: true },
            { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
            { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          ],
          data: encodeClaimData(
            rewardEpoch,
            amount,
            claimQuote.claim.proof,
          ),
        }),
      );

      const signature = await sendTransaction(transaction, connection, {
        skipPreflight: false,
      });

      setClaimStatus(`Submitted ${short(signature, 10)}. Confirming...`);
      const confirmation = await connection.confirmTransaction(
        signature,
        "confirmed",
      );

      if (confirmation.value.err) {
        throw new Error(`Claim transaction failed: ${JSON.stringify(confirmation.value.err)}`);
      }

      setClaimStatus(
        `Claim confirmed: ${claimQuote.claim.amount} Devnet test reward units.`,
      );
    } catch (cause: unknown) {
      setClaimStatus("Claim failed.");
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setClaiming(false);
    }
  }

  const rewardsConfigured = Boolean(
    config?.rewards_program_id && config?.reward_mint,
  );

  return (
    <main className="shell">
      <section className="panel">
        <div className="eyebrow">SOLANA EDGE · DEVNET</div>
        <h1>Useful Work Miner</h1>
        <p className="lede">
          Your browser processes deterministic Solana data jobs. Verified work
          receives a signed test receipt. No Mainnet token or real-value reward
          exists in this prototype.
        </p>

        <div className="walletRow">
          <WalletMultiButton />
          <span className={connected ? "badge live" : "badge"}>
            {connected && publicKey ? short(publicKey.toBase58(), 5) : "wallet disconnected"}
          </span>
          <span className={rewardsConfigured ? "badge live" : "badge"}>
            {rewardsConfigured ? "rewards deployed" : "rewards not deployed"}
          </span>
        </div>

        <div className="controls">
          <label>
            Contribution mode
            <select
              value={mode}
              onChange={(event) => setMode(event.target.value as Mode)}
              disabled={running}
            >
              <option value="eco">Eco · long cooldown</option>
              <option value="balanced">Balanced</option>
              <option value="max">Max · short cooldown</option>
            </select>
          </label>

          <div className="buttonRow">
            <button
              className="primary"
              onClick={() => void start()}
              disabled={running || !wasmReady || !connected}
            >
              {running ? "Contributing..." : "Start contributing"}
            </button>
            <button className="secondary" onClick={stop} disabled={!running}>
              Stop
            </button>
          </div>
        </div>

        <div className="status">
          <span className={running ? "dot active" : "dot"} />
          {status}
        </div>

        {error ? <div className="error">{error}</div> : null}

        <div className="metrics">
          <article>
            <span>Completed</span>
            <strong>{jobsCompleted}</strong>
          </article>
          <article>
            <span>Verified</span>
            <strong>{verifiedJobs}</strong>
          </article>
          <article>
            <span>Test score</span>
            <strong>{score}</strong>
          </article>
        </div>

        {lastReceipt ? (
          <div className="receipt">
            <div className="receiptTitle">Last signed work receipt</div>
            <dl>
              <div>
                <dt>Receipt</dt>
                <dd>{short(lastReceipt.receipt.receipt_id, 10)}</dd>
              </div>
              <div>
                <dt>Slot</dt>
                <dd>{lastReceipt.receipt.source_slot}</dd>
              </div>
              <div>
                <dt>Reward epoch</dt>
                <dd>{lastReceipt.receipt.reward_epoch}</dd>
              </div>
              <div>
                <dt>Score</dt>
                <dd>{lastReceipt.receipt.score}</dd>
              </div>
              <div>
                <dt>Signer</dt>
                <dd>{short(lastReceipt.signer_pubkey, 8)}</dd>
              </div>
            </dl>
          </div>
        ) : null}

        <section className="claimPanel">
          <div>
            <div className="receiptTitle">Devnet reward claim</div>
            <p className="safety">
              An epoch must first be aggregated and published on-chain by the
              protocol operator. Claiming is the first step that requests a
              Solana transaction from your wallet.
            </p>
          </div>

          <label>
            Reward epoch
            <input
              inputMode="numeric"
              value={claimEpoch}
              onChange={(event) => setClaimEpoch(event.target.value)}
              placeholder="e.g. 497000"
              disabled={claiming}
            />
          </label>

          <div className="buttonRow">
            <button
              className="secondary"
              onClick={() => void loadReward()}
              disabled={!connected || claiming}
            >
              Load proof
            </button>
            <button
              className="primary"
              onClick={() => void claimReward()}
              disabled={!connected || !claimQuote || !rewardsConfigured || claiming}
            >
              {claiming ? "Claiming..." : "Claim test reward"}
            </button>
          </div>

          <div className="claimStatus">{claimStatus}</div>
          {claimQuote ? (
            <div className="claimMeta">
              <span>Amount: {claimQuote.claim.amount}</span>
              <span>Proof nodes: {claimQuote.claim.proof.length}</span>
              <span>Root: {short(claimQuote.merkle_root_hex, 9)}</span>
            </div>
          ) : null}
        </section>

        <p className="safety">
          Mining stops when you press Stop, close the page, or move the tab out
          of the foreground. Worker authentication signs a message only; reward
          claiming is a separate explicit Devnet transaction.
        </p>
      </section>
    </main>
  );
}
