import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  WATCHLIST,
  seedSymbol,
  nextCandle,
  quoteFromCandles,
  mockPositions,
  type Candle,
  type FeedStatus,
} from "@qoin/core";

// Same contract as apps/web/lib/useMarketFeed.ts — swap the interval for a real
// WebSocket connection to go live on both platforms without touching consumers.
export function useMarketFeed() {
  const [status, setStatus] = useState<FeedStatus>("connecting");
  const [candlesBySymbol, setCandlesBySymbol] = useState<Record<string, Candle[]>>(() => {
    const seeded: Record<string, Candle[]> = {};
    WATCHLIST.forEach((s) => (seeded[s] = seedSymbol(s)));
    return seeded;
  });
  const staleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const connectTimeout = setTimeout(() => setStatus("live"), 600);
    const tickInterval = setInterval(() => {
      setCandlesBySymbol((prev) => {
        const next: Record<string, Candle[]> = { ...prev };
        WATCHLIST.forEach((symbol) => {
          const series = prev[symbol];
          next[symbol] = [...series.slice(1), nextCandle(series[series.length - 1])];
        });
        return next;
      });
      setStatus("live");
      if (staleTimer.current) clearTimeout(staleTimer.current);
      staleTimer.current = setTimeout(() => setStatus("stale"), 5000);
    }, 2000);

    return () => {
      clearTimeout(connectTimeout);
      clearInterval(tickInterval);
      if (staleTimer.current) clearTimeout(staleTimer.current);
    };
  }, []);

  const quotes = Object.fromEntries(
    WATCHLIST.map((s) => [s, quoteFromCandles(s, candlesBySymbol[s])])
  );

  return { status, candlesBySymbol, quotes };
}

export function usePositions(quotes: ReturnType<typeof useMarketFeed>["quotes"]) {
  return useQuery({
    queryKey: ["positions", Object.values(quotes).map((q) => q.price).join(",")],
    queryFn: async () => mockPositions(quotes),
    placeholderData: (prev) => prev,
  });
}
