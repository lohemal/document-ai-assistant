//! 이 PC 에 대해 알아야 할 것들 — 메모리, 저장공간, Ollama 가 깔려 있는지.
//!
//! 모두 **알아내지 못할 수 있다**. 그럴 때는 `None` 을 돌려주고, 부르는 쪽이
//! 무난한 쪽을 고르게 한다. 못 알아냈다고 해서 앱이 멈추면 안 된다.

use std::path::PathBuf;

/// 이 PC 에 깔린 메모리 (GB). 알아내지 못하면 None.
#[cfg(windows)]
pub fn ram_gb() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 {
        return None;
    }
    // 반올림한다. 16GB PC 가 15GB 로 나와 15 로 보이면 엉뚱한 것을 권하게 된다.
    let bytes = status.ullTotalPhys;
    Some(((bytes as f64) / 1024.0 / 1024.0 / 1024.0).round() as u64)
}

#[cfg(not(windows))]
pub fn ram_gb() -> Option<u64> {
    None
}

/// 그 폴더가 있는 드라이브에 남은 공간 (바이트).
#[cfg(windows)]
pub fn free_bytes(path: &std::path::Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    // 폴더가 아직 없을 수 있으니 있는 데까지 거슬러 올라간다
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        match probe.parent() {
            Some(p) => probe = p.to_path_buf(),
            None => return None,
        }
    }

    let wide: Vec<u16> = probe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        None
    } else {
        Some(free)
    }
}

#[cfg(not(windows))]
pub fn free_bytes(_path: &std::path::Path) -> Option<u64> {
    None
}

/// Ollama 가 모델을 담아 두는 곳. 저장공간을 잴 때 이 드라이브를 본다.
pub fn ollama_models_dir() -> Option<PathBuf> {
    // OLLAMA_MODELS 로 옮겨 둔 사람도 있다
    if let Ok(custom) = std::env::var("OLLAMA_MODELS") {
        if !custom.trim().is_empty() {
            return Some(PathBuf::from(custom));
        }
    }
    std::env::var("USERPROFILE")
        .ok()
        .map(|home| PathBuf::from(home).join(".ollama").join("models"))
}

/// Ollama 실행 파일이 이 PC 에 있는가. 있으면 그 자리를 돌려준다.
///
/// **이걸로 "쓸 수 있다" 를 판단하지는 않는다.** 깔려 있어도 실행 중이 아닐 수
/// 있다. 실제로 쓸 수 있는지는 API 를 불러 봐야 안다 (`ollama::probe`).
/// 이 함수는 "안 깔림" 과 "깔렸는데 안 켜짐" 을 가르는 데만 쓴다.
pub fn ollama_binary() -> Option<PathBuf> {
    let mut spots: Vec<PathBuf> = Vec::new();

    for (var, tail) in [
        ("LOCALAPPDATA", "Programs/Ollama/ollama.exe"),
        ("ProgramFiles", "Ollama/ollama.exe"),
        ("ProgramFiles(x86)", "Ollama/ollama.exe"),
    ] {
        if let Ok(base) = std::env::var(var) {
            spots.push(PathBuf::from(base).join(tail));
        }
    }

    // PATH 에 잡혀 있을 수도 있다
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            spots.push(dir.join("ollama.exe"));
            spots.push(dir.join("ollama"));
        }
    }

    spots.into_iter().find(|p| p.is_file())
}

/// winget 이 이 PC 에 있는가.
///
/// 학교 PC 에는 없을 수 있고, 있어도 정책으로 막혀 있을 수 있다. 그래서
/// **앱이 winget 을 대신 돌리지는 않는다.** 있을 때만 명령을 보여 주고,
/// 사용자가 직접 붙여 넣게 한다.
pub fn has_winget() -> bool {
    std::env::var("PATH")
        .ok()
        .map(|path| {
            std::env::split_paths(&path).any(|d| d.join("winget.exe").is_file())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 메모리를_잰다() {
        // 이 PC 에서는 알아낼 수 있어야 한다. 못 알아내도 죽지는 않는다.
        if let Some(gb) = ram_gb() {
            assert!(gb >= 1 && gb <= 4096, "메모리 값이 이상합니다: {gb}GB");
        }
    }

    #[test]
    fn 저장공간을_잰다() {
        let dir = std::env::temp_dir();
        if let Some(free) = free_bytes(&dir) {
            assert!(free > 0);
        }
    }

    #[test]
    fn 아직_없는_폴더의_저장공간도_잰다() {
        // 모델 폴더는 Ollama 를 깔기 전에는 없다
        let missing = std::env::temp_dir().join("docaid-없는폴더").join("더-없는-폴더");
        assert!(!missing.exists());
        // 거슬러 올라가 드라이브를 찾으므로 값이 나와야 한다
        if cfg!(windows) {
            assert!(free_bytes(&missing).is_some());
        }
    }

    #[test]
    fn 모델_폴더_자리를_안다() {
        if cfg!(windows) {
            assert!(ollama_models_dir().is_some());
        }
    }
}
