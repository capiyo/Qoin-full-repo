"use client";

import { useState } from "react";
import type { Side, TickerQuote } from "@qoin/core";

export function OrderTicket({ symbol, quote }: { symbol: string; quote: TickerQuote }) {
  const [side, setSide] = useState<Side>("buy");
  const [type, setType] = useState<"market" | "limit">("market");
  const [size, setSize] = useState("");
  const [limitPrice, setLimitPrice] = useState("");
  const [confirming, setConfirming] = useState(false);
  const [placed, setPlaced] = useState<string | null>(null);

  const sizeNum = parseFloat(size) || 0;
  const priceForEstimate = type === "limit" ? parseFloat(limitPrice) || quote.price : quote.price;
  const estimate = sizeNum * priceForEstimate;
  const valid = sizeNum > 0 && (type === "market" || parseFloat(limitPrice) > 0);

  function submit() {
    if (!valid) return;
    setConfirming(true);
  }

  function confirm() {
    setConfirming(false);
    setPlaced(
      `${side === "buy" ? "Bought" : "Sold"} ${size} ${symbol} ${
        type === "limit" ? `@ $${limitPrice}` : "at market"
      }`
    );
    setSize("");
    setLimitPrice("");
    setTimeout(() => setPlaced(null), 4000);
  }

  return (
    <div className="qoin-card p-4">
      <div className="flex rounded-lg overflow-hidden border border-qoin-border mb-3" role="tablist" aria-label="Order side">
        {(["buy", "sell"] as Side[]).map((s) => (
          <button
            key={s}
            role="tab"
            aria-selected={side === s}
            onClick={() => setSide(s)}
            className={`qoin-focus flex-1 py-2 text-sm font-semibold uppercase transition-colors ${
              side === s
                ? s === "buy"
                  ? "bg-qoin-bullish/15 text-qoin-bullish"
                  : "bg-qoin-bearish/15 text-qoin-bearish"
                : "text-qoin-text-muted hover:text-qoin-text-primary"
            }`}
          >
            {s}
          </button>
        ))}
      </div>

      <div className="flex gap-2 mb-3 text-xs">
        {(["market", "limit"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setType(t)}
            className={`qoin-focus px-2.5 py-1 rounded-md font-medium capitalize ${
              type === t ? "bg-qoin-bg-primary text-qoin-accent-cyan" : "text-qoin-text-muted"
            }`}
          >
            {t}
          </button>
        ))}
      </div>

      <label className="text-xs text-qoin-text-muted" htmlFor="size">
        Size ({symbol.split("/")[0]})
      </label>
      <input
        id="size"
        inputMode="decimal"
        value={size}
        onChange={(e) => setSize(e.target.value)}
        placeholder="0.00"
        className="qoin-focus tabular w-full mt-1 mb-3 rounded-lg bg-qoin-bg-primary border border-qoin-border px-3 py-2 text-sm"
      />

      {type === "limit" && (
        <>
          <label className="text-xs text-qoin-text-muted" htmlFor="limitPrice">
            Limit price (USD)
          </label>
          <input
            id="limitPrice"
            inputMode="decimal"
            value={limitPrice}
            onChange={(e) => setLimitPrice(e.target.value)}
            placeholder={quote.price.toFixed(2)}
            className="qoin-focus tabular w-full mt-1 mb-3 rounded-lg bg-qoin-bg-primary border border-qoin-border px-3 py-2 text-sm"
          />
        </>
      )}

      <div className="flex justify-between text-xs text-qoin-text-muted mb-3">
        <span>Est. total</span>
        <span className="tabular">${estimate.toFixed(2)}</span>
      </div>

      {!confirming ? (
        <button
          onClick={submit}
          disabled={!valid}
          className={`qoin-focus w-full rounded-lg py-2.5 text-sm font-semibold transition-opacity ${
            side === "buy" ? "bg-qoin-bullish text-qoin-bg-primary" : "bg-qoin-bearish text-qoin-bg-primary"
          } disabled:opacity-40`}
        >
          {side === "buy" ? "Buy" : "Sell"} {symbol}
        </button>
      ) : (
        <div className="rounded-lg border border-qoin-border p-3">
          <p className="text-xs text-qoin-text-muted mb-2">
            Confirm: {side} {size} {symbol} {type === "limit" ? `@ $${limitPrice}` : "at market"}?
          </p>
          <div className="flex gap-2">
            <button
              onClick={confirm}
              className="qoin-focus flex-1 rounded-lg bg-qoin-accent-cyan text-qoin-bg-primary text-sm font-semibold py-2"
            >
              Confirm
            </button>
            <button
              onClick={() => setConfirming(false)}
              className="qoin-focus flex-1 rounded-lg border border-qoin-border text-sm font-medium py-2"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {placed && <p className="mt-3 text-xs text-qoin-bullish">{placed}</p>}
    </div>
  );
}
