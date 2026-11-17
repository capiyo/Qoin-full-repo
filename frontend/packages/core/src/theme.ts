// Shared Qoin forest-workspace brand tokens for web, native and desktop clients.
export const qoinColors = {
  bgPrimary: "#0e1612",
  bgSecondary: "#14221a",
  bgRaised: "#19291f",
  bgSoft: "#203329",
  border: "#2b4032",
  accent: "#70d99d",
  accentStrong: "#43bd7a",
  bullish: "#70d99d",
  bearish: "#e47a82",
  warning: "#e0b45f",
  textPrimary: "#e6f1e9",
  textMuted: "#9cb2a2",
  white: "#ffffff",
} as const;

export type QoinColorToken = keyof typeof qoinColors;

export const fonts = {
  heading: "'Manrope', sans-serif",
  body: "'DM Sans', sans-serif",
} as const;
