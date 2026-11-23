export type Section = 'overview' | 'backup' | 'applications' | 'drivers' | 'cloud' | 'restore' | 'history' | 'settings'
export type ModuleId = 'apps' | 'vscode' | 'chrome' | 'git' | 'node'

export interface InstalledApp {
  name: string
  id: string
  version: string
  source: string
  supported: boolean
}

export interface AppScan {
  apps: InstalledApp[]
  warnings: string[]
}

export interface DriverDevice {
  name: string
  manufacturer: string
  provider: string
  version: string
  deviceId: string
  inf: string
  problemCode: number
}

export interface DriverScan {
  devices: DriverDevice[]
  issues: number
}

export interface BackupRequest {
  destination: string
  modules: ModuleId[]
  appIds: string[]
}

export interface BackupResult {
  archivePath: string
  fileCount: number
  bytesWritten: number
  warnings: string[]
}

export interface Verification {
  verified: boolean
  filesChecked: number
  errors: string[]
}
