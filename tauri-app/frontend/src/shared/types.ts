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
