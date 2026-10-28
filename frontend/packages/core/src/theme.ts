// Single source of truth for Qoin brand tokens.
// Consumed by apps/web/tailwind.config.ts and apps/mobile/src/theme/.

export const qoinColors = {
  bgPrimary: "#0B0E14",   // app background — near-black, low glare
  bgSecondary: "#131722", // card/panel background
  border: "#1E222D",      // dividers, panel borders
  accentCyan: "#2FD8E8",  // brand accent — links, focus, active nav, primary buttons
  bullish: "#22C55E",     // up candles, positive PnL, buy
  bearish: "#EF4444",     // down candles, negative PnL, sell/destructive
  warning: "#F59E0B",     // stale data / risk limit warnings only
  textPrimary: "#D1D4DC", // soft off-white body text
  textMuted: "#787B86",   // secondary text, labels, disabled
  white: "#FFFFFF",       // logo mark only
} as const;

export type QoinColorToken = keyof typeof qoinColors;

export const fonts = {
  heading: "'Space Grotesk', sans-serif",
  body: "'Inter', sans-serif",
} as const;
