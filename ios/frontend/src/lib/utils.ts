import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function extractErrorMessage(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  // tauri 插件 reject 的是普通对象（如 BiometricPrompt 取消的 {code,message}），
  // 直接 String() 会得到 "[object Object]"——优先取 message 字段
  if (typeof e === 'object' && e !== null) {
    const msg = (e as { message?: unknown }).message
    if (typeof msg === 'string' && msg) return msg
  }
  return String(e)
}

const memoryFallback = new Map<string, string>()

const safeStorage = {
  get(key: string): string | null {
    try { return localStorage.getItem(key) } catch { return memoryFallback.get(key) ?? null }
  },
  set(key: string, value: string): boolean {
    try { localStorage.setItem(key, value); memoryFallback.set(key, value); return true } catch { memoryFallback.set(key, value); return false }
  },
}

export { safeStorage }
