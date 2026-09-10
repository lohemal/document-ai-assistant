import { invoke } from '@tauri-apps/api/core'

export type AppInfo = {
  displayName: string
  version: string
  dataDir: string
  /** 그 안의 자리들 — 백업할 때 무엇을 챙겨야 하는지 그대로 보여 준다 */
  dbPath: string
  filesDir: string
  backupsDir: string
  /** AI 모델이 저장되는 곳 (Ollama 폴더). 앱 자료와 다른 곳이다 */
  modelsDir: string | null
  /** 자료를 정상적으로 열었는가. 못 열었어도 앱은 뜬다. */
  storageReady: boolean
  storageError: string | null
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info')
}

export function openDataDir(): Promise<void> {
  return invoke('app_open_data_dir')
}

export type BackupOut = { path: string; bytes: number }

/** data.db 를 백업 폴더에 한 파일로 복사한다 (PDF 사본 폴더는 따로 복사해야 한다) */
export function backupNow(): Promise<BackupOut> {
  return invoke<BackupOut>('app_backup_now')
}
