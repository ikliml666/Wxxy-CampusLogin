export interface UpdateAvailableData {
  has_update: boolean
  latest_version: string
  release_notes?: string
}

export interface UpdateInfo {
  has_update: boolean
  latest_version: string
  release_notes: string
  assets: { name: string; url: string; size: number }[]
  sha256_checksum?: string
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
