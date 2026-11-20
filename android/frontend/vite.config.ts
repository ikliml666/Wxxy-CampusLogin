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
          // 主包曾因此超 500kB 警告线：这些 vendor 此前全部内联进 index（实测 @tauri-apps
          // 107kB + i18next 系 102kB + tailwind-merge 70kB + lucide 40kB）。按依赖分桶
          // 让应用代码独占 index——本地加载总量不变，缓存粒度更稳。
          // 注意不收 @vladmandic/human：它是 faceService 动态 import 的惰性边界，
          // manualChunks 会把它拉进静态 chunk，首屏就背上 1.5MB 的 tfjs
          if (id.includes('node_modules/@tauri-apps/')) return 'vendor-tauri'
          if (id.includes('node_modules/i18next/') || id.includes('node_modules/react-i18next/') || id.includes('node_modules/i18next-browser-languagedetector/')) return 'vendor-i18n'
          if (id.includes('node_modules/lucide-react/') || id.includes('node_modules/tailwind-merge/') || id.includes('node_modules/zustand/') || id.includes('node_modules/use-sync-external-store/')) return 'vendor-misc'
        },
      },
    },
  },
})
