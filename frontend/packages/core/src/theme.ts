// Shared Qoin brand tokens for web and native clients.
export const qoinColors = {
  bgPrimary: "#F7F9FC",
  bgSecondary: "#FFFFFF",
  border: "#E6EBF2",
  accentCyan: "#2867DC",
  bullish: "#2867DC",
  bearish: "#E05A67",
  warning: "#D99528",
  textPrimary: "#24344D",
  textMuted: "#8794A7",
  white: "#FFFFFF",
} as const;

export type QoinColorToken = keyof typeof qoinColors;

export const fonts = {
  heading: "'Manrope', sans-serif",
  body: "'DM Sans', sans-serif",
} as const;
