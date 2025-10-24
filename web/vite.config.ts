import { defineConfig } from 'vite'

export default defineConfig({
  server: {
    port: 5173,
    open: '/index.html',
    middlewareMode: false,
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
})
