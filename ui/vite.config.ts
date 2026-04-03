import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [tailwindcss()],
  resolve: {
    alias: { "@": "/src" },
  },
  server: {
    proxy: {
      "/api": "http://localhost:3000",
      "/dev": "http://localhost:3000",
      "/health": "http://localhost:3000",
      "/ready": "http://localhost:3000",
    },
  },
});
