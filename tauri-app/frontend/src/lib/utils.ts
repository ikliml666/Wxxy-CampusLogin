import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function extractErrorMessage(e: unknown): string {
  return typeof e === 'string' ? e : (e instanceof Error ? e.message : String(e))
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
