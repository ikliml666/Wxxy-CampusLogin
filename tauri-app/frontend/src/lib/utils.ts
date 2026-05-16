import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function extractErrorMessage(e: unknown): string {
  return typeof e === 'string' ? e : (e instanceof Error ? e.message : String(e))
}

const safeStorage = {
  get(key: string): string | null {
    try { return localStorage.getItem(key) } catch { return null }
  },
  set(key: string, value: string): boolean {
    try { localStorage.setItem(key, value); return true } catch { return false }
  },
}

export { safeStorage }
