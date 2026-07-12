# Pixel Studio 像素工作室

Rust + egui 圖片編輯與轉檔工具，由 Photo Tool 複刻並擴充進階編輯功能。預設繁體中文介面，可切換 English。

## 功能一覽

### 檢視與瀏覽
- Windows 全螢幕截圖、開啟單張圖片、資料夾瀏覽、拖放開檔、剪貼簿貼上
- 預覽區滑鼠滾輪縮放、符合視窗、100% 檢視
- 檔案資訊面板

### 圖片編輯
- 快速標註工作區：裁切、文字、線條、箭頭、框線與儲存副本
- 旋轉（左 / 右 / 180°）、水平 / 垂直翻轉
- 調整尺寸、裁切、局部選取（矩形 / 圓形）
- 亮度、對比、飽和度、曝光、模糊、銳利化
- 灰階、反相、**自動對比**
- **進階濾鏡**：色相、復古、暈影、色溫、色調、色階（Posterize）
- 編輯復原 / 重做

快速標註快捷鍵：`Ctrl+S` 儲存 `.pixel-project`，`Ctrl+Shift+S` 儲存圖片副本，`Enter` 套用裁切，`Esc` 取消裁切。

### 圖層與合成
- 文字 / 圖片圖層、浮水印、字型選擇與預覽
- 拖曳、對齊、圖層順序、複製、隱藏
- 匯出合成圖、儲存 / 開啟 `.pixel-project` 專案檔

### 轉檔與批次
- JPEG / PNG / WEBP / BMP / TIFF 互轉
- 資料夾批次轉檔、可選縮放與浮水印
- GIF 動畫製作
- 360° GPano JPEG 匯出

### C2PA 內容憑證
- 檢視 manifest、單張加入 / 移除、批次處理

### 多語系
- 繁體中文（預設）/ English，於「檔案」面板切換

## 執行

```powershell
cargo run -p studio-app
```

## 開發驗證

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## CLI

```powershell
cargo run -p studio-cli -- info input.jpg
cargo run -p studio-cli -- convert input.png output.jpg --format jpg --quality 90
cargo run -p studio-cli -- batch input-folder output-folder --format webp --quality 90
cargo run -p studio-cli -- c2pa-remove input-folder output-folder
cargo run -p studio-cli -- gif input-folder output.gif --delay-ms 120
cargo run -p studio-cli -- pano360 input.jpg output_360.jpg --mode pad --width 2048
```

## 發行建置

```powershell
.\scripts\build-release.ps1 -RunTests
```

輸出：`dist\studio-app.exe`、`dist\studio-cli.exe`
