// useAppStore 已拆分为领域 store，此文件仅保留 re-export 以兼容旧引用
export { useAppInit } from './useAppInit'
export { hasPendingConfig, flushPendingConfig } from './useConfigStore'
