import type { Config } from "tailwindcss";
import { qoinColors, fonts } from "@qoin/core";

const config: Config = {
  darkMode: ["class"],
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        "qoin-bg-primary": qoinColors.bgPrimary,
        "qoin-bg-secondary": qoinColors.bgSecondary,
        "qoin-border": qoinColors.border,
        "qoin-accent-cyan": qoinColors.accentCyan,
        "qoin-bullish": qoinColors.bullish,
        "qoin-bearish": qoinColors.bearish,
        "qoin-warning": qoinColors.warning,
        "qoin-text-primary": qoinColors.textPrimary,
        "qoin-text-muted": qoinColors.textMuted,
        "qoin-white": qoinColors.white,
      },
      fontFamily: {
        heading: [fonts.heading],
        body: [fonts.body],
      },
    },
  },
  plugins: [],
};

export default config;
