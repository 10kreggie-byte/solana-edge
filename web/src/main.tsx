import React from "react";
import ReactDOM from "react-dom/client";
import { ConnectionProvider, WalletProvider } from "@solana/wallet-adapter-react";
import { WalletModalProvider } from "@solana/wallet-adapter-react-ui";
import "@solana/wallet-adapter-react-ui/styles.css";
import App from "./App";
import "./styles.css";

const DEVNET_RPC = "https://api.devnet.solana.com";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ConnectionProvider endpoint={DEVNET_RPC}>
      <WalletProvider wallets={[]} autoConnect>
        <WalletModalProvider>
          <App />
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  </React.StrictMode>,
);
