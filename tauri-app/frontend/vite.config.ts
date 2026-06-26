import path from 'path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
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
        manualChunks(id) {
          if (id.includes('node_modules/react/') || id.includes('node_modules/react-dom/')) return 'vendor-react'
          if (id.includes('node_modules/framer-motion/')) return 'vendor-motion'
          if (id.includes('node_modules/gsap/')) return 'vendor-gsap'
          if (id.includes('node_modules/@radix-ui/')) return 'vendor-radix'
        },
      },
    },
  },
})
