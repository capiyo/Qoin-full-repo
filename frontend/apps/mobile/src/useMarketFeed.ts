import { useMemo, useState } from "react";
import {
  WATCHLIST,
  seedSymbol,
  quoteFromCandles,
  type Candle,
} from "@qoin/core";

// Static demo snapshot only. This hook deliberately does not simulate a live
// exchange connection or advance prices. Replace the snapshot with the shared
// authenticated market-data stream when that provider is available.
export function useMarketFeed() {
  const [candlesBySymbol] = useState<Record<string, Candle[]>>(() => {
    const seeded: Record<string, Candle[]> = {};
    WATCHLIST.forEach((symbol) => {
      seeded[symbol] = seedSymbol(symbol);
    });
    return seeded;
  });

  const quotes = useMemo(
    () => Object.fromEntries(
      WATCHLIST.map((symbol) => [symbol, quoteFromCandles(symbol, candlesBySymbol[symbol])]),
    ),
    [candlesBySymbol],
  );

  return { status: "disconnected" as const, candlesBySymbol, quotes };
}
