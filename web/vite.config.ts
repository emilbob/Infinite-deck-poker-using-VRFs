import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // The wasm package is a file: dependency rebuilt by `npm run build:wasm`.
  // Excluding it from pre-bundling means a rebuilt .wasm is picked up on
  // reload rather than served from a stale optimize cache.
  optimizeDeps: { exclude: ['Poker_VRF'] },
})
