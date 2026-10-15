# Pixel Studio 像素工作室

[English](README.en.md) ｜ 繁體中文

用 Rust + [Slint](https://slint.dev) 打造的圖片編輯器，目標是**取代並超越 Windows 小畫家**：繪圖、圖層、貼圖、社群貼文圖、離線自動打碼、批次轉檔，支援 Windows、Linux、macOS。

下載：[Releases](https://github.com/stevenke1981/pixel-studio/releases)
- Windows：`pixel-studio-setup-windows-x64.exe`（安裝程式）或 `pixel-studio-windows-x64.zip`（免安裝）
- Linux：`pixel-studio-linux-x64.tar.gz`
- macOS（Intel 與 Apple 晶片）：`pixel-studio-macos-universal-app.zip`

![主視窗](docs/slint-main.png)
![貼圖庫](docs/slint-stickers.png)
![自動打碼：偵測到的文字框，紅框為預選的敏感資料](docs/slint-redact.png)

## v0.3.0：介面改用 Slint

- GUI 從 egui 改為 Slint（Windows 使用 Skia 繪製，避免軟體繪製器局部重繪時視窗透明的問題；Skia 無法初始化時自動退回軟體繪製器）。
- 畫布以像素緩衝區顯示，筆刷只重新合成「有變動的矩形區域」；圖層在變動時才重新繪製。
- 文件邏輯（繪圖、圖層、復原、貼圖、打碼、專案檔）移到與介面無關的 `doc.rs`，並有單元測試。
- 快捷鍵：Ctrl+O/S/Z/Y/V/C/A/D/G、Ctrl+Shift+V 貼為圖層、Del 刪除圖層、方向鍵微調圖層（Shift ×10）、Enter 裁切、Esc 取消、+/-/0/1 縮放、Ctrl+滾輪縮放、中鍵或「平移」工具拖曳畫面；可直接把圖片拖進視窗開啟。

## 和小畫家比較

| 功能 | 小畫家 | Pixel Studio |
|---|---|---|
| 鉛筆、筆刷、橡皮擦、油漆桶、滴管、文字 | ✓ | ✓（另有螢光筆、噴槍、書法筆、馬賽克筆） |
| 形狀：直線、箭頭、矩形、橢圓、三角形 | ✓ | ✓（外框用色彩 1、可用色彩 2 填滿；形狀是可再編輯的圖層） |
| 選取、裁切、調整大小、旋轉、翻轉 | ✓ | ✓（矩形／圓形選取、選取區裁切、縮放影像與調整畫布大小） |
| 縮放、復原／重做 | ✓ | ✓（放大時像素清晰顯示＋像素格線 Ctrl+G） |
| 圖層、透明背景 | 部分 | ✓ 文字／圖片／形狀／貼圖圖層，可移動、排序、隱藏、調整不透明度 |
| 亮度、對比、飽和度、色相、模糊、銳利化 | ✗ | ✓ 另有曝光、色溫、色調、復古、暈影、色階、自動對比、灰階、反相 |
| 貼圖庫（箭頭、對話框、愛心星星、勾叉、數字、標示框、表情、NEW/HOT/SALE 標籤、社群通用圖示） | ✗ | ✓ 65 個原創 SVG，可搜尋、點選或拖曳插入、縮放、換色、外框、陰影 |
| 社群貼文畫布尺寸 | ✗ | ✓ IG 貼文 1080×1080、IG 直式 1080×1350、IG 限動 1080×1920、FB 1200×630、X 1600×900、YouTube 縮圖 1280×720、LINE 1040×1040 |
| 快速文字樣式（外框、陰影標題） | ✗ | ✓ |
| 自動打碼 | ✗ | ✓ 離線文字偵測，自動預選電話、Email、身分證、信用卡、網址、地址 |
| 剪貼簿貼上成新圖片、螢幕截圖 | 部分 | ✓ |
| 格式 | PNG/JPEG/BMP/GIF/TIFF/HEIC | PNG、JPEG、BMP、GIF、ICO、WebP（無損＋有損品質）、TIFF |
| 批次轉檔、GIF 動畫、360° 全景、C2PA 內容憑證 | ✗ | ✓ |
| 右鍵選單、開啟檔案清單 | 系統內建 | ✓「用 Pixel Studio 編輯／開啟」，不搶預設程式 |

## 自動打碼（「自動打碼」分頁）

1. 按「偵測文字」：在本機用 [ocrs](https://github.com/robertknight/ocrs) 模型偵測文字（安裝版與壓縮檔已內附模型，約 12 MB；從原始碼執行時第一次會自動下載）。
2. 偵測到的文字框會畫在圖上。辨識出電話（台灣與國際）、Email、台灣身分證（含檢查碼）、信用卡（Luhn 驗證）、網址、地址的框會自動預選（紅色）。
3. 可點框切換、右鍵刪框、用「打碼框」工具拖曳新增框、在清單勾選，或用關鍵字搜尋選取。
4. 選擇打碼方式：**實心色塊（預設，可選顏色）**、馬賽克或模糊，並設定外擴邊距。馬賽克和模糊有時可被部分還原，機密資料請用實心色塊。
5. 套用後會把圖層合併進像素（破壞性、無法從檔案還原）。匯出時預設移除 EXIF／XMP，可在「工具」或「自動打碼」分頁勾選保留。
6. 另有「框選」與「打碼筆刷」可手動打碼。

**限制：** 文字「偵測」不分語言，中文和英文都能框出來；但文字「辨識」目前只認得英文字母、數字和符號，所以純中文內容（例如中文地址）不會被自動預選，需要手動點選。

命令列：`studio-cli redact 輸入.png 輸出.png [--style solid|mosaic|blur] [--keyword 字詞] [--all] [--keep-metadata]`

## 右鍵選單與檔案關聯

- 安裝程式可勾選：右鍵選單、加入「開啟檔案」清單（預設勾選，不變更預設程式）、設為 PNG/WebP 預設程式（預設不勾）、桌面捷徑。
- 免安裝版：執行 `pixel-studio --register-shell`（只寫入目前使用者，Windows 用 HKCU），移除用 `--unregister-shell`。
- Linux：建立 `.desktop`（開啟方式）、Nautilus 指令碼、Dolphin 服務選單、Nemo 動作。macOS：Finder 快速動作，`.app` 也宣告了圖片文件類型。
- 參數：`pixel-studio [--edit|--view] 檔案`

## 語言

繁體中文（中文系統預設）、English（其他語系預設）。可在「設定」分頁切換（會記住），或設定環境變數 `PIXEL_STUDIO_LANG=en`。

## 從原始碼建置

```bash
cargo build --release          # 產生 target/release/pixel-studio 與 studio-cli
./scripts/fetch-models.sh      # 選用：下載 OCR 模型到 models/
cargo test --workspace
```

Linux 需要 GTK3 等套件（見 `.github/workflows/ci.yml`）。

## 圖示授權

所有貼圖與程式圖示都是本專案原創（MIT 授權）。**不包含任何平台商標**（Facebook、Instagram、X、LINE、YouTube 等），社群類別使用通用圖示（讚、分享、留言、播放、鈴鐺、相機、標註、主題標籤、地點）。

## 授權

MIT
