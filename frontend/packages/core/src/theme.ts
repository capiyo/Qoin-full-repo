// Shared Qoin brand tokens for web and native clients.
export const qoinColors = {
  bgPrimary: "#F5F8FC",
  bgSecondary: "#FFFFFF",
  border: "#E5EBF3",
  accentCyan: "#246BDF",
  accent: "#246BDF",
  bullish: "#16845B",
  bearish: "#D94B59",
  warning: "#C88924",
  textPrimary: "#1B2A41",
  textMuted: "#7D8BA1",
  white: "#FFFFFF",
} as const;

export type QoinColorToken = keyof typeof qoinColors;

export const fonts = {
  heading: "'Manrope', sans-serif",
  body: "'DM Sans', sans-serif",
} as const;
