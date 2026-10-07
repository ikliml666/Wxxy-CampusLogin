/**
 * 2D 人脸验证的命令式弹窗桥：验证门（tauriApi.verifyWindowsIdentity，非组件层）
 * 需要弹出相机 UI 并等待结果——组件渲染与逻辑解耦的惯用桥接（open 存 store、
 * Dialog 单例挂在 App 根部、openFaceDialog 返回 Promise）。
 *
 * 判定 helper 也放这里（faceVerifyStore 不依赖 tauriApi，避免循环引用：
 * tauriApi → 本 store → useConfigStore）。
 */

import { create } from 'zustand'
import { useConfigStore } from '@/hooks/useConfigStore'
import { hasTemplate } from './faceService'

export type FaceDialogMode = 'enroll' | 'verify'
export type FaceDialogResult = { ok: boolean; reason?: string }

interface FaceDialogState {
  open: boolean
  mode: FaceDialogMode
  resolve: ((r: FaceDialogResult) => void) | null
  openFaceDialog: (mode: FaceDialogMode) => Promise<FaceDialogResult>
  closeFaceDialog: (r: FaceDialogResult) => void
}

export const useFaceDialogStore = create<FaceDialogState>((set, get) => ({
  open: false,
  mode: 'verify',
  resolve: null,
  openFaceDialog: (mode) => new Promise((resolve) => {
    if (get().open) {
      // 并发触发（两门同时到期）：按取消处理，后到者走常规验证失败路径
      resolve({ ok: false, reason: 'cancel' })
      return
    }
    set({ open: true, mode, resolve })
  }),
  closeFaceDialog: (r) => {
    get().resolve?.(r)
    set({ open: false, resolve: null })
  },
}))

/** 验证门是否应走 2D 人脸回退：开关开启且已录入人脸模板 */
export function shouldUseFaceFallback(): boolean {
  return useConfigStore.getState().config.allow2dFaceVerify === true && hasTemplate()
}
