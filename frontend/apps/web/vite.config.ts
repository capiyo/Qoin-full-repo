import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, ".", "");
  return {
    plugins: [react()],
    define: {
      "globalThis.__QOIN_API_URL__": JSON.stringify(env.VITE_QOIN_API_URL ?? ""),
    },
    server: { port: 5173 },
  };
});
