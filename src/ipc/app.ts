import { invoke } from '@tauri-apps/api/core'

export type AppInfo = {
  displayName: string
  version: string
  dataDir: string
}

type RawAppInfo = {
  display_name: string
  version: string
  data_dir: string
}

export async function getAppInfo(): Promise<AppInfo> {
  const raw = await invoke<RawAppInfo>('app_info')
  return {
    displayName: raw.display_name,
    version: raw.version,
    dataDir: raw.data_dir,
  }
}
