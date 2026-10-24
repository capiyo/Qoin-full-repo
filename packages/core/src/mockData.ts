// Mock market-data engine: random-walk candle generator + a fake "live feed" hook
// that both apps subscribe to. Replace the interval-based push below with a real
// WebSocket `onmessage` handler to go live — keep the same return shape.

import type { Candle, TickerQuote, Position, FeedStatus } from "./types";

export const WATCHLIST = ["BTC/USD", "ETH/USD", "SOL/USD", "AVAX/USD"] as const;

const BASE_PRICES: Record<string, number> = {
  "BTC/USD": 64250,
  "ETH/USD": 3180,
  "SOL/USD": 148,
  "AVAX/USD": 27.4,
};

function seedCandles(symbol: string, count = 120): Candle[] {
  let price = BASE_PRICES[symbol] ?? 100;
  const now = Math.floor(Date.now() / 1000);
  const candles: Candle[] = [];
  for (let i = count; i > 0; i--) {
    const open = price;
    const drift = (Math.random() - 0.5) * open * 0.006;
    const close = Math.max(0.01, open + drift);
    const high = Math.max(open, close) + Math.random() * open * 0.002;
    const low = Math.min(open, close) - Math.random() * open * 0.002;
    candles.push({
      time: now - i * 60,
      open,
      high,
      low,
      close,
      volume: Math.random() * 1000,
    });
    price = close;
  }
  return candles;
}

export function nextCandle(prev: Candle): Candle {
  const open = prev.close;
  const drift = (Math.random() - 0.5) * open * 0.006;
  const close = Math.max(0.01, open + drift);
  const high = Math.max(open, close) + Math.random() * open * 0.002;
  const low = Math.min(open, close) - Math.random() * open * 0.002;
  return {
    time: prev.time + 60,
    open,
    high,
    low,
    close,
    volume: Math.random() * 1000,
  };
}

export function seedSymbol(symbol: string) {
  return seedCandles(symbol);
}

export function quoteFromCandles(symbol: string, candles: Candle[]): TickerQuote {
  const last = candles[candles.length - 1];
  const dayAgo = candles[Math.max(0, candles.length - 24)];
  const changePct24h = ((last.close - dayAgo.open) / dayAgo.open) * 100;
  const volume24h = candles.slice(-24).reduce((sum, c) => sum + c.volume, 0);
  return { symbol, price: last.close, changePct24h, volume24h };
}

export function mockPositions(quotes: Record<string, TickerQuote>): Position[] {
  const seed: Array<Omit<Position, "markPrice" | "pnl" | "pnlPct">> = [
    { id: "p1", symbol: "BTC/USD", side: "buy", size: 0.25, entryPrice: 61800, leverage: 3 },
    { id: "p2", symbol: "ETH/USD", side: "sell", size: 4, entryPrice: 3320, leverage: 5 },
    { id: "p3", symbol: "SOL/USD", side: "buy", size: 40, entryPrice: 139.5, leverage: 2 },
  ];
  return seed.map((p) => {
    const mark = quotes[p.symbol]?.price ?? p.entryPrice;
    const dir = p.side === "buy" ? 1 : -1;
    const pnl = (mark - p.entryPrice) * p.size * dir;
    const pnlPct = ((mark - p.entryPrice) / p.entryPrice) * 100 * dir;
    return { ...p, markPrice: mark, pnl, pnlPct };
  });
}

export type FeedTick = {
  status: FeedStatus;
  candlesBySymbol: Record<string, Candle[]>;
  quotes: Record<string, TickerQuote>;
};
