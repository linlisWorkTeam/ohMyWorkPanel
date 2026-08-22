use std::path::Path;
use std::process::Command;

use crate::db::AppResult;

/// Try to locate tesseract executable: PATH first, then known install paths.
fn find_tesseract() -> Option<String> {
    // Check PATH
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("tesseract.exe");
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    // Common install locations
    for base in [
        r"C:\Program Files\Tesseract-OCR",
        r"C:\Program Files (x86)\Tesseract-OCR",
    ] {
        let candidate = Path::new(base).join("tesseract.exe");
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

/// Run OCR on an image file and return extracted text.
pub fn ocr_image(image_path: &str) -> AppResult<String> {
    let exe = find_tesseract().ok_or_else(|| {
        "未找到 Tesseract OCR。请从 https://github.com/UB-Mannheim/tesseract/wiki 安装。".to_string()
    })?;

    let output = Command::new(&exe)
        .arg(image_path)
        .arg("stdout")
        .arg("-l")
        .arg("chi_sim+eng")
        .output()
        .map_err(|e| format!("无法启动 Tesseract：{e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Tesseract 识别失败：{}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        return Err("OCR 未能从图片中识别出文字。".into());
    }
    Ok(text)
}
/// Run OCR from a base64-encoded image string.
/// Supports both raw base64 and data URLs (e.g. data:image/png;base64,...).
pub fn ocr_image_base64(base64_data: &str) -> AppResult<String> {
    use std::io::Write;
    use base64::Engine;

    // Strip data URL prefix if present
    let b64 = if let Some(comma) = base64_data.find(',') {
        &base64_data[comma + 1..]
    } else {
        base64_data
    };

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Base64 解码失败: {e}"))?;

    let dir = std::env::temp_dir();
    let path = dir.join(format!("ohmyworkpanel_ocr_{}.png", crate::db::id()));
    let mut file = std::fs::File::create(&path).map_err(|e| format!("创建临时文件失败: {e}"))?;
    file.write_all(&bytes).map_err(|e| format!("写入临时文件失败: {e}"))?;
    drop(file);

    let result = ocr_image(&path.to_string_lossy());
    let _ = std::fs::remove_file(&path);
    result
}
