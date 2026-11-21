import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, ".", "");
  const gateway = env.QOIN_GATEWAY_URL || "http://127.0.0.1:8080";
  return {
    plugins: [react()],
    server: {
      port: 5173,
      proxy: {
        "/api": { target: gateway, changeOrigin: true, ws: true },
        "/health": { target: gateway, changeOrigin: true },
      },
    },
  };
});
