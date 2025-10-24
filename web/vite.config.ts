import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'
import wasm from 'vite-plugin-wasm'

export default defineConfig({
  plugins: [wasm(), react()],
  server: {
    port: 5173,
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
    exclude: ['transmitwave-wasm'],
  },
  resolve: {
    alias: {
      'transmitwave-wasm': path.resolve(__dirname, 'node_modules/transmitwave-wasm'),
      'env': path.resolve(__dirname, 'wasm-env-shim.js')
    }
  },
})
