export interface UpdateAvailableData {
  hasUpdate: boolean
  latestVersion: string
  releaseNotes?: string
}

export interface UpdateInfo {
  hasUpdate: boolean
  latestVersion: string
  releaseNotes: string
  assets: { name: string; url: string; size: number }[]
  sha256Checksum?: string
  /** 最近一次更新检查失败原因（本次检查前快照；null=此前无失败或上次已成功） */
  lastCheckError: string | null
  /** 最近一次更新检查完成时间 unix ms（null=从未检查） */
  lastCheckTime: number | null
}

export interface DownloadProgress {
  downloaded: number
  total: number
  speed: number
  percent: number
}

export interface MirrorSource {
  name: string
  url: string
  description: string
}
