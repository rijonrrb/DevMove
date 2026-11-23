import { invoke, isTauri } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type { AppScan, BackupRequest, BackupResult, DriverScan, ExtractionResult, Verification } from './types'

export const desktopAvailable = isTauri()

export async function scanInstalledApps(): Promise<AppScan> {
  return invoke<AppScan>('scan_apps')
}
export async function scanDeviceDrivers(): Promise<DriverScan> {
  return invoke<DriverScan>('scan_drivers')
}
export async function createArchive(request: BackupRequest): Promise<BackupResult> {
  return invoke<BackupResult>('create_backup', { request })
}
export async function verifyArchive(path: string): Promise<Verification> {
  return invoke<Verification>('verify_backup', { path })
}

export async function extractArchive(archivePath: string, destination: string): Promise<ExtractionResult> {
  return invoke<ExtractionResult>('extract_archive', { archivePath, destination })
}

export async function selectDirectory(): Promise<string | null> {
  const result = await open({ directory: true, multiple: false, title: 'Choose a backup destination' })
  return typeof result === 'string' ? result : null
}
export async function selectArchive(): Promise<string | null> {
  const result = await open({
    multiple: false,
    directory: false,
    title: 'Choose a DevMove ZIP backup',
    filters: [{ name: 'DevMove archives', extensions: ['zip'] }],
  })
  return typeof result === 'string' ? result : null
}
