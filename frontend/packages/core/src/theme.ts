// Single source of truth for Qoin's white-and-blue brand system.
// Shared by the web and React Native applications.
export const qoinColors = {
  bgPrimary: "#F5F8FC",
  bgSecondary: "#FFFFFF",
  border: "#E5EBF3",
  accentCyan: "#246BDF",
  bullish: "#246BDF",
  bearish: "#D94B59",
  warning: "#C88924",
  textPrimary: "#1B2A41",
  textMuted: "#7D8BA1",
  white: "#FFFFFF",
} as const;

export type QoinColorToken = keyof typeof qoinColors;

export const fonts = {
  heading: "'Space Grotesk', sans-serif",
  body: "'Inter', sans-serif",
} as const;
