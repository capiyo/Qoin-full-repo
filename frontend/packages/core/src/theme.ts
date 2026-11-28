// Shared Qoin brand tokens for web and native clients.
export const qoinColors = {
  bgPrimary: "#0E1612",
  bgSecondary: "#17291F",
  border: "#365247",
  accentCyan: "#7DF162",
  accent: "#7DF162",
  bullish: "#7DF162",
  bearish: "#E05A67",
  warning: "#D99528",
  textPrimary: "#EDF4EE",
  textMuted: "#89968D",
  white: "#FFFFFF",
} as const;

export type QoinColorToken = keyof typeof qoinColors;

export const fonts = {
  heading: "'Manrope', sans-serif",
  body: "'DM Sans', sans-serif",
} as const;
