// Single source of truth for Qoin brand tokens.
// Consumed by apps/web/tailwind.config.ts and apps/mobile/src/theme/.

export const qoinColors = {
  bgPrimary: "#171522",   // app background — near-black, low glare
  bgSecondary: "#211E2D", // card/panel background
  border: "#393346",      // dividers, panel borders
  accentCyan: "#C6B7FF",  // brand accent — links, focus, active nav, primary buttons
  bullish: "#A8D7BD",     // up candles, positive PnL, buy
  bearish: "#EFA7A7",     // down candles, negative PnL, sell/destructive
  warning: "#F0B98B",     // stale data / risk limit warnings only
  textPrimary: "#F3EFF8", // soft off-white body text
  textMuted: "#A79DB4",   // secondary text, labels, disabled
  white: "#FFF9F2",       // logo mark only
} as const;

export type QoinColorToken = keyof typeof qoinColors;

export const fonts = {
  heading: "'Space Grotesk', sans-serif",
  body: "'Inter', sans-serif",
} as const;
