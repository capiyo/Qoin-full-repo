// Synthetic market fixtures for UI development and tests only.
// These values are not exchange observations and must never be used for
// portfolio decisions, risk calculations, or order execution.

import type { Candle, TickerQuote, Position, FeedStatus } from "./types";

export const WATCHLIST = ["BTC/USD", "ETH/USD", "SOL/USD", "AVAX/USD"] as const;

const BASE_PRICES: Record<string, number> = {
  "BTC/USD": 64250,
  "ETH/USD": 3180,
  "SOL/USD": 148,
  "AVAX/USD": 27.4,
};

function symbolSeed(symbol: string): number {
  return Array.from(symbol).reduce((seed, character) => (seed * 31 + character.charCodeAt(0)) >>> 0, 7);
}

function seedCandles(symbol: string, count = 120): Candle[] {
  let price = BASE_PRICES[symbol] ?? 100;
  const now = Math.floor(Date.now() / 1000);
  const seed = symbolSeed(symbol);
  const candles: Candle[] = [];

  for (let i = count; i > 0; i--) {
    const open = price;
    const phase = seed * 0.0001 + i * 0.73;
    const wave = Math.sin(phase) * 0.65 + Math.cos(phase * 0.37) * 0.35;
    const drift = wave * open * 0.0018;
    const close = Math.max(0.01, open + drift);
    const spread = (0.0003 + Math.abs(Math.sin(phase * 1.7)) * 0.0012) * open;

    candles.push({
      time: now - i * 60,
      open,
      high: Math.max(open, close) + spread,
      low: Math.max(0.01, Math.min(open, close) - spread),
      close,
      volume: 250 + Math.abs(Math.sin(phase * 2.3)) * 750,
    });
    price = close;
  }
  return candles;
}

// Deterministic synthetic continuation for chart-component tests only.
export function nextCandle(prev: Candle): Candle {
  const phase = prev.time * 0.017 + prev.close * 0.0001;
  const wave = Math.sin(phase);
  const open = prev.close;
  const close = Math.max(0.01, open + wave * open * 0.0018);
  const spread = (0.0003 + Math.abs(Math.cos(phase)) * 0.0012) * open;

  return {
    time: prev.time + 60,
    open,
    high: Math.max(open, close) + spread,
    low: Math.max(0.01, Math.min(open, close) - spread),
    close,
    volume: 250 + Math.abs(Math.sin(phase * 2.3)) * 750,
  };
}

export function seedSymbol(symbol: string) {
  return seedCandles(symbol);
}

export function quoteFromCandles(symbol: string, candles: Candle[]): TickerQuote {
  const last = candles[candles.length - 1];
  const dayAgo = candles[Math.max(0, candles.length - 24)];
  const changePct24h = ((last.close - dayAgo.open) / dayAgo.open) * 100;
  const volume24h = candles.slice(-24).reduce((sum, candle) => sum + candle.volume, 0);
  return { symbol, price: last.close, changePct24h, volume24h };
}

export function mockPositions(quotes: Record<string, TickerQuote>): Position[] {
  const seed: Array<Omit<Position, "markPrice" | "pnl" | "pnlPct">> = [
    { id: "p1", symbol: "BTC/USD", side: "buy", size: 0.25, entryPrice: 61800, leverage: 3 },
    { id: "p2", symbol: "ETH/USD", side: "sell", size: 4, entryPrice: 3320, leverage: 5 },
    { id: "p3", symbol: "SOL/USD", side: "buy", size: 40, entryPrice: 139.5, leverage: 2 },
  ];
  return seed.map((position) => {
    const mark = quotes[position.symbol]?.price ?? position.entryPrice;
    const direction = position.side === "buy" ? 1 : -1;
    const pnl = (mark - position.entryPrice) * position.size * direction;
    const pnlPct = ((mark - position.entryPrice) / position.entryPrice) * 100 * direction;
    return { ...position, markPrice: mark, pnl, pnlPct };
  });
}

export type FeedTick = {
  status: FeedStatus;
  candlesBySymbol: Record<string, Candle[]>;
  quotes: Record<string, TickerQuote>;
};
