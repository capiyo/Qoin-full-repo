"use client";

import { useState } from "react";
import { Shell } from "@/components/Shell";

export default function SettingsPage() {
  const [riskWarnings, setRiskWarnings] = useState(true);
  const [confirmOrders, setConfirmOrders] = useState(true);

  return (
    <Shell>
      <h1 className="font-heading text-xl font-semibold mb-6">Settings</h1>
      <div className="qoin-card p-5 max-w-lg flex flex-col gap-5">
        <div className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium">Risk limit warnings</div>
            <div className="text-xs text-qoin-text-muted">
              Show a warning banner when a position nears its risk limit.
            </div>
          </div>
          <button
            role="switch"
            aria-checked={riskWarnings}
            onClick={() => setRiskWarnings((v) => !v)}
            className={`qoin-focus h-6 w-11 rounded-full transition-colors ${
              riskWarnings ? "bg-qoin-accent-cyan" : "bg-qoin-border"
            }`}
          >
            <span
              className={`block h-5 w-5 bg-qoin-white rounded-full transition-transform ${
                riskWarnings ? "translate-x-5" : "translate-x-0.5"
              }`}
            />
          </button>
        </div>

        <div className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium">Confirm before placing orders</div>
            <div className="text-xs text-qoin-text-muted">Adds a confirmation step to market and limit orders.</div>
          </div>
          <button
            role="switch"
            aria-checked={confirmOrders}
            onClick={() => setConfirmOrders((v) => !v)}
            className={`qoin-focus h-6 w-11 rounded-full transition-colors ${
              confirmOrders ? "bg-qoin-accent-cyan" : "bg-qoin-border"
            }`}
          >
            <span
              className={`block h-5 w-5 bg-qoin-white rounded-full transition-transform ${
                confirmOrders ? "translate-x-5" : "translate-x-0.5"
              }`}
            />
          </button>
        </div>
      </div>
    </Shell>
  );
}
