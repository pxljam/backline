import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: true,
    port: 5173,
    // In native development the API runs alongside; in Docker, Caddy handles it.
    proxy: {
      "/api": { target: process.env.API_URL ?? "http://localhost:8080", changeOrigin: true },
      "/ical": { target: process.env.API_URL ?? "http://localhost:8080", changeOrigin: true },
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test-setup.ts"],
  },
});
