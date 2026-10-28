"use client";

import { useState } from "react";
import { WATCHLIST } from "@qoin/core";
import { Shell } from "@/components/Shell";
import { StatusBadge } from "@/components/StatusBadge";
import { FeedBanner } from "@/components/FeedBanner";
import { TradingChart } from "@/components/TradingChart";
import { OrderTicket } from "@/components/OrderTicket";
import { useMarketFeed } from "@/lib/useMarketFeed";

function fmtPrice(n: number) {
  return n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

export default function DashboardPage() {
  const { status, candlesBySymbol, quotes } = useMarketFeed();
  const [active, setActive] = useState<string>(WATCHLIST[0]);

  return (
    <Shell>
      <div className="flex items-center justify-between mb-4">
        <h1 className="font-heading text-xl font-semibold">Dashboard</h1>
        <StatusBadge status={status} />
      </div>

      <FeedBanner status={status} />

      <div className="grid grid-cols-1 lg:grid-cols-5 gap-4">
        <div className="lg:col-span-1 flex flex-col gap-2">
          {WATCHLIST.map((symbol) => {
            const q = quotes[symbol];
            const up = q.changePct24h >= 0;
            return (
              <button
                key={symbol}
                onClick={() => setActive(symbol)}
                className={`qoin-focus qoin-card text-left p-3 transition-colors ${
                  active === symbol ? "ring-1 ring-qoin-accent-cyan" : ""
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="font-medium text-sm">{symbol}</span>
                  <span className={`text-xs font-medium tabular ${up ? "text-qoin-bullish" : "text-qoin-bearish"}`}>
                    {up ? "+" : ""}
                    {q.changePct24h.toFixed(2)}%
                  </span>
                </div>
                <div className="tabular text-lg font-semibold mt-1">${fmtPrice(q.price)}</div>
              </button>
            );
          })}
        </div>

        <div className="lg:col-span-3 qoin-card p-4">
          <div className="flex items-center justify-between mb-3">
            <div>
              <div className="font-heading text-lg font-semibold">{active}</div>
              <div className="tabular text-2xl font-bold">${fmtPrice(quotes[active].price)}</div>
            </div>
            <div
              className={`tabular text-sm font-medium ${
                quotes[active].changePct24h >= 0 ? "text-qoin-bullish" : "text-qoin-bearish"
              }`}
            >
              {quotes[active].changePct24h >= 0 ? "+" : ""}
              {quotes[active].changePct24h.toFixed(2)}% (24h)
            </div>
          </div>
          <TradingChart candles={candlesBySymbol[active]} height={380} />
        </div>

        <div className="lg:col-span-1">
          <OrderTicket symbol={active} quote={quotes[active]} />
        </div>
      </div>
    </Shell>
  );
}
