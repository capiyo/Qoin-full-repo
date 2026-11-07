import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

const qoinEnv = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;
const qoinRuntime = globalThis as typeof globalThis & { __QOIN_API_URL?: string; __QOIN_WS_URL?: string };
qoinRuntime.__QOIN_API_URL = qoinEnv?.VITE_QOIN_API_URL;
qoinRuntime.__QOIN_WS_URL = qoinEnv?.VITE_QOIN_WS_URL;

const root = document.getElementById("root");
if (!root) throw new Error("Missing #root element");
createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
