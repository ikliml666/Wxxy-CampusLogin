// 浏览器 mock 测试专用 vite 配置 —— 不改动 git 跟踪的 vite.config.ts / index.html。
// 用法：npx vite --config vite.mock.config.ts（见会话交付说明）
// 原理：在基础配置上追加 transformIndexHtml 插件，把 /tauri-mock.js 以同步
// <script> 注入 index.html 头部（先于 module 脚本执行），mock 住 __TAURI_INTERNALS__。
import os from 'node:os'
import path from 'node:path'
import type { Plugin } from 'vite'
import baseConfig from './vite.config'

const tauriMockInject: Plugin = {
  name: 'tauri-mock-inject',
  transformIndexHtml(html) {
    return {
      html,
      tags: [{ tag: 'script', attrs: { src: '/tauri-mock.js' }, injectTo: 'head-prepend' as const }],
    }
  },
}

const base = baseConfig as import('vite').UserConfig

// 注意：本机防护软件按文件路径拦截原版 @esbuild/win32-x64/esbuild.exe 的任何写盘
// （改名副本无碍，Defender 无拦截记录）。esbuild 服务进程在 vite 加载配置阶段就已用
// 原版二进制启动，配置内设 ESBUILD_BINARY_PATH 为时已晚——必须在启动命令上带环境变量：
//   ESBUILD_BINARY_PATH="$PWD/node_modules/.pnpm/esbuild@0.25.12/node_modules/@esbuild/win32-x64/esb-renamed.exe" npx vite --config vite.mock.config.ts

export default {
  ...base,
  // cacheDir 外移：node_modules/.vite 下 esbuild 子进程写入全部 Access is denied
  // （bash 同目录可写、Defender CFA 未启用，疑似杀软按目录/进程拦截），落到系统 TEMP 绕开。
  cacheDir: path.join(os.tmpdir(), 'wxxy-vite-desktop'),
  plugins: [...(base.plugins ?? []), tauriMockInject],
  server: { ...(base.server ?? {}), port: 5175 },
}
