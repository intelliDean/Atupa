import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: '../bin/atupa/dist', // Output to the bin directory so RustEmbed can pick it up
    emptyOutDir: true,
  }
})
