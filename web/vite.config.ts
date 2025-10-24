import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'
import wasm from 'vite-plugin-wasm'

export default defineConfig({
  plugins: [wasm(), react()],
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
      external: ['env'],
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
