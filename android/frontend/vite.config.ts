import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const dirname = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(dirname, './src'),
    },
  },
  base: './',
  clearScreen: false,
  server: {
    port: 5174,
    strictPort: true,
    watch: {
      // IDE/工具原子保存会生成 .*.tmpdir 临时目录，Windows 上 fs.watch 盯到它即 EBUSY 闪退
      ignored: ['**/.*.tmpdir/**', '**/.*.tmpdir'],
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
    minify: 'esbuild',
    cssMinify: true,
    target: 'es2020',
    modulePreload: { polyfill: false },
    rollupOptions: {
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
