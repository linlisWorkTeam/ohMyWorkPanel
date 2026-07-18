# OCR 图片文字识别

## 概述

通过 Tesseract OCR 引擎（v5.4.0）从图片中提取文字，支持中英文混合识别。

## 依赖

- Tesseract-OCR 5.4.0（安装在 C:\\Program Files\\Tesseract-OCR\	esseract.exe）
- 语言包：chi_sim（简体中文）+ eng（英文）

## 架构

| 层 | 文件 | 说明 |
|---|---|---|
| Rust 后端 | src-tauri/src/ocr.rs | 定位 tesseract 可执行文件，调用 CLI 识别图片，返回文本 |
| Tauri 命令 | src-tauri/src/commands.rs | 注册 ocr_image 命令供前端调用 |
| 前端 API | src/api.ts | pi.ocrImage(imagePath) 封装 Tauri invoke |
| UI 入口 | src/App.tsx | 发���框旁的 📷 按钮，调用 handleOcr() |

## 使用方式

1. 点击输入框左侧的 📷 按钮
2. 从文件对话框选择图片（支持 png/jpg/jpeg/bmp/gif/tiff/webp）
3. OCR 识别结果自动插入到输入框光标处
4. 按需编辑后发送

## 技术细节

- 运行时通过 PATH 或固定安装路径寻找 	esseract.exe
- 命令行参数：	esseract <image_path> stdout -l chi_sim+eng
- 若 Tesseract 未安装，前端会显示引导安装提示
