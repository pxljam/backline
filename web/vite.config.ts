import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: true,
    port: 5173,
    // En developpement natif, l'API tourne a cote ; en Docker, Caddy s'en charge.
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
