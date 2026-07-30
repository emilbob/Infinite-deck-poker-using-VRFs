import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  // '/' is correct for Render and for local builds, which both serve from a
  // domain root. VITE_BASE is kept as an escape hatch for hosts that serve
  // from a subpath — GitHub Pages did, from /<repo>/, and set it accordingly.
  base: process.env.VITE_BASE ?? '/',
  plugins: [react(), tailwindcss()],
  // The wasm package is a file: dependency rebuilt by `npm run build:wasm`.
  // Excluding it from pre-bundling means a rebuilt .wasm is picked up on
  // reload rather than served from a stale optimize cache.
  optimizeDeps: { exclude: ['Poker_VRF'] },
  server: {
    fs: {
      // `Poker_VRF/pkg` is a symlinked file: dependency one level above this
      // root. Without this, dev serves poker_vrf.js but 403s the .wasm it
      // fetches, so init() rejects and the page silently never deals.
      // `vite build` is unaffected — it copies the asset into dist/.
      allow: ['..'],
    },
  },
})
