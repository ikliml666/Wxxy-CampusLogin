export interface PortalStatusResult {
  online: boolean
  message?: string
  reachable?: boolean
  loginAvailable?: boolean
}

export interface CommandResult {
  success: boolean
  message?: string
  data?: Record<string, unknown>
}

export interface LoginResult extends CommandResult {}
