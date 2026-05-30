import path from 'path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { readFileSync } from 'fs'

const host = process.env.TAURI_DEV_HOST

const tauriConf = JSON.parse(readFileSync(path.resolve(__dirname, '../src-tauri/tauri.conf.json'), 'utf-8'))
const appVersion = tauriConf.version || '0.0.0'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  define: {
    '__APP_VERSION__': JSON.stringify(appVersion),
  },
  base: './',
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? {
      protocol: 'ws',
      host,
      port: 5174,
    } : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
    minify: 'esbuild',
    cssMinify: true,
    cssCodeSplit: true,
    reportCompressedSize: false,
    target: 'es2021',
    modulePreload: {
      polyfill: false,
    },
    rollupOptions: {
      maxParallelFileOps: 8,
      output: {
        manualChunks: {
          'vendor-react': ['react', 'react-dom'],
          'vendor-motion': ['framer-motion'],
          'vendor-gsap': ['gsap'],
          'vendor-radix': [
            '@radix-ui/react-dialog',
            '@radix-ui/react-select',
            '@radix-ui/react-tooltip',
            '@radix-ui/react-switch',
          ],
        },
      },
    },
  },
})
