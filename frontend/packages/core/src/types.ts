export type Side = "buy" | "sell";

export interface Candle {
  time: number; // unix seconds
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface TickerQuote {
  symbol: string;
  price: number;
  changePct24h: number;
  volume24h: number;
}

export interface Position {
  id: string;
  symbol: string;
  side: Side;
  size: number;
  entryPrice: number;
  markPrice: number;
  leverage: number;
  pnl: number;
  pnlPct: number;
}

export interface OrderTicket {
  symbol: string;
  side: Side;
  type: "market" | "limit";
  size: number;
  limitPrice?: number;
}

export type FeedStatus = "connecting" | "live" | "stale" | "disconnected";

export interface OrderFormState {
  side: Side;
  type: "market" | "limit";
  size: string;
  limitPrice: string;
}
