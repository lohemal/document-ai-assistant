fn main() {
    // 아이콘을 바꿔도 다시 빌드되도록 알려 준다.
    // (방과후 앱에서 이걸 빠뜨려 설치 파일만 새 아이콘이 되고 앱은 옛 아이콘이
    //  남는 일이 있었다.)
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/32x32.png");
    println!("cargo:rerun-if-changed=icons/128x128.png");
    println!("cargo:rerun-if-changed=tauri.conf.json");

    tauri_build::build()
}
