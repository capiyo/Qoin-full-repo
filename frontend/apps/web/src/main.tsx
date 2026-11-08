import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { initializeQoinFirebase } from "@qoin/core";
import "./styles.css";

const qoinEnv = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;
const qoinRuntime = globalThis as typeof globalThis & { __QOIN_API_URL?: string; __QOIN_WS_URL?: string };
qoinRuntime.__QOIN_API_URL = qoinEnv?.VITE_QOIN_API_URL;
qoinRuntime.__QOIN_WS_URL = qoinEnv?.VITE_QOIN_WS_URL;
if (qoinEnv?.VITE_FIREBASE_API_KEY && qoinEnv?.VITE_FIREBASE_AUTH_DOMAIN && qoinEnv?.VITE_FIREBASE_PROJECT_ID && qoinEnv?.VITE_FIREBASE_STORAGE_BUCKET && qoinEnv?.VITE_FIREBASE_MESSAGING_SENDER_ID && qoinEnv?.VITE_FIREBASE_APP_ID) {
  initializeQoinFirebase({ apiKey: qoinEnv.VITE_FIREBASE_API_KEY, authDomain: qoinEnv.VITE_FIREBASE_AUTH_DOMAIN, projectId: qoinEnv.VITE_FIREBASE_PROJECT_ID, storageBucket: qoinEnv.VITE_FIREBASE_STORAGE_BUCKET, messagingSenderId: qoinEnv.VITE_FIREBASE_MESSAGING_SENDER_ID, appId: qoinEnv.VITE_FIREBASE_APP_ID });
}

const root = document.getElementById("root");
if (!root) throw new Error("Missing #root element");
createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
