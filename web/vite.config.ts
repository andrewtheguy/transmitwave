import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    open: '/index.html',
    middlewareMode: false,
    allowedHosts: ['.trycloudflare.com'],
    fs: {
      // Allow serving from parent directories and node_modules
      allow: ['..'],
    },
  },
  build: {
    target: 'esnext',
    minify: 'terser',
    sourcemap: true,
    outDir: 'dist',
    rollupOptions: {
      output: {
        entryFileNames: '[name].[hash].js',
        chunkFileNames: '[name].[hash].js',
        assetFileNames: '[name].[hash][extname]',
      },
    },
  },
  optimizeDeps: {
    exclude: ['testaudio-wasm'],
  },
  resolve: {
    alias: {
      'testaudio-wasm': path.resolve(__dirname, 'node_modules/testaudio-wasm'),
    },
  },
})
