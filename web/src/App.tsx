import { useEffect, useRef, useState } from "react";
import { useWallet } from "@solana/wallet-adapter-react";
import { WalletMultiButton } from "@solana/wallet-adapter-react-ui";
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

type Mode = "eco" | "balanced" | "max";

const API_URL = (
  import.meta.env.VITE_COORDINATOR_URL ?? "http://127.0.0.1:8787"
).replace(/\/$/, "");

const MODE_DELAY_MS: Record<Mode, number> = {
  eco: 15_000,
  balanced: 5_000,
  max: 1_500,
};

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function api<T>(
  path: string,
  options?: RequestInit,
): Promise<T> {
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
  const { publicKey, signMessage, connected } = useWallet();
  const [wasmReady, setWasmReady] = useState(false);
  const [running, setRunning] = useState(false);
  const [mode, setMode] = useState<Mode>("balanced");
  const [status, setStatus] = useState("Initializing WebAssembly...");
  const [jobsCompleted, setJobsCompleted] = useState(0);
  const [verifiedJobs, setVerifiedJobs] = useState(0);
  const [score, setScore] = useState(0);
  const [lastReceipt, setLastReceipt] = useState<SignedWorkReceipt | null>(null);
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
  }, [publicKey?.toBase58()]);

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

        <p className="safety">
          Mining stops when you press Stop, close the page, or move the tab out
          of the foreground. The wallet signs an authentication message only;
          this UI does not request a transaction.
        </p>
      </section>
    </main>
  );
}
