export interface PortalStatusResult {
  online: boolean
  message?: string
  reachable?: boolean
  loginAvailable?: boolean
}

export interface CommandResult {
  success: boolean
  message?: string
}

export interface LoginResult extends CommandResult {}
