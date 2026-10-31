import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import { tanstackRouter } from "@tanstack/router-plugin/vite"
import react from "@vitejs/plugin-react"
import { type Plugin, defineConfig } from "vite"

const API_TARGET = "http://127.0.0.1:7878"

/** `vite --mode mock` serves MSW's worker script from /mock instead of shipping it in public/. */
function mockServiceWorker(): Plugin {
  return {
    name: "lp-mock-service-worker",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/mockServiceWorker.js", (_req, res) => {
        res.setHeader("content-type", "text/javascript")
        res.end(
          readFileSync(
            resolve(import.meta.dirname, "mock/mockServiceWorker.js")
          )
        )
      })
    },
  }
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: [
    ...(mode === "mock" ? [mockServiceWorker()] : []),
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true,
      routesDirectory: "src/routes",
      generatedRouteTree: "src/routeTree.gen.ts",
    }),
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      // Same-origin /api in dev. SSE needs no timeout and no buffering.
      "/api": {
        target: API_TARGET,
        changeOrigin: false,
        timeout: 0,
        proxyTimeout: 0,
        configure: (proxy) => {
          proxy.on("proxyReq", (proxyReq) => {
            proxyReq.setHeader("Host", new URL(API_TARGET).host)
            proxyReq.setHeader("Origin", API_TARGET)
          })
          proxy.on("proxyRes", (proxyRes) => {
            if (
              proxyRes.headers["content-type"]?.includes("text/event-stream")
            ) {
              proxyRes.headers["cache-control"] = "no-cache"
              proxyRes.headers["x-accel-buffering"] = "no"
            }
          })
        },
      },
    },
  },
  build: { sourcemap: false, chunkSizeWarningLimit: 600 },
}))
