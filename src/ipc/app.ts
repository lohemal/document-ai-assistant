import { invoke } from '@tauri-apps/api/core'

export type AppInfo = {
  displayName: string
  version: string
  dataDir: string
  /** 자료를 정상적으로 열었는가. 못 열었어도 앱은 뜬다. */
  storageReady: boolean
  storageError: string | null
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info')
}
