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

## v0.4.1：單框刪除與編輯資料保護

自動／手動打碼框可逐一選取、刪除及復原，勾選套用與單框編輯分開；同時修正未儲存保護、原子寫入、手勢取消及過期 OCR 結果。詳見 [繁中發行說明與使用限制](https://github.com/stevenke1981/pixel-studio/blob/v0.4.1/docs/releases/v0.4.1.zh-TW.md)。新增功能的 Windows 原生滑鼠／鍵盤與 GUI 匯出尚未完整驗收，測試後端及 CI 截圖不等同原生操作驗收。

## v0.4.0：圖示按鈕、選單列、Windows 中文字型、體積縮小

- **全部改為 SVG 圖示按鈕**（頂列、工具盤、浮動工具列、調整、圖層、貼圖、打碼、批次）：滑鼠停留會顯示名稱與快捷鍵；有滑過、按下、停用、啟用中四種狀態，並提供螢幕閱讀器標籤。圖示集是原創的（`scripts/gen-icons.py`，共 115 個）。
- **Windows 風格選單列**：檔案(F)、編輯(E)、檢視(V)、影像(I)、圖層(L)、工具(T)、說明(H)。可用 Alt+字母開啟，方向鍵移動、Enter 執行、Esc 關閉，選單內會顯示快捷鍵。包含最近的檔案、列印、內容、剪下/刪除、尺規、狀態列、全螢幕、影像大小/畫布大小對話框、快捷鍵一覽（F1）、關於。
- **新快捷鍵**：工具單鍵（S P B L A K E F I T M C H U R、Shift+R），X 交換顏色，Ctrl+X/E/P/R/W/J/]/[，F11。
- **Windows 中文字型**：介面字型會指定 Microsoft JhengHei UI，沒有的話依序改用 YaHei UI、PMingLiU、SimSun，「啟」這類字會正常顯示，所以按鈕恢復為「開啟」。
- **執行檔體積**：Linux 版 pixel-studio 從 80.5 MB 降到 33.7 MB，studio-cli 從 45.0 MB 降到 17.7 MB。

![選單列](docs/menu-bar.png)
![圖示提示](docs/tooltip.png)
![圖示集](docs/icons.png)

## v0.3.1：快速工具列、C2PA 對話框、時間戳檔名

- **浮動快速工具列**：復原、重做、快存（Ctrl+S，第一次才詢問格式/品質，之後直接覆寫）、另存、複製、貼上、裁切到選取、旋轉、符合視窗 / 100%、格線（Ctrl+G）。可拖曳把手移動、停靠或隱藏（頂列「顯示/隱藏工具列」），位置會記住；按鈕有含快捷鍵的提示，無法使用時會停用。
- **畫布右鍵選單**：選取、裁切、文字、形狀工具下按右鍵，可做復原/重做、複製/貼上、全選、裁切、刪除圖層、旋轉、縮放、格線、儲存。
- **C2PA 內容憑證對話框**（資訊分頁 →「C2PA 內容憑證…」）：顯示驗證狀態、manifest 數量、標題/作者/動作與原始 JSON；可加入憑證並另存目前影像、儲存時寫入 C2PA、移除 C2PA 另存（副檔名與來源相同，不會覆寫來源）；也能批次加入/移除，顯示進度與結果。開檔時會用現有 manifest 預填（沒有就用檔名當標題）。
- **預設檔名**：儲存、另存、匯出、截圖與貼上成新圖時，預設檔名為 `pixelstudio_YYYYMMDD_HHMMSS.<副檔名>`（當地時間），仍可修改。CLI 的 convert/redact/crop/blur-region/pano360 輸出參數改為選填，省略時也用這個格式。
- 打碼框可拖曳移動，拖右下角可調整大小；選取工具下可拖畫布右緣、下緣或角落的把手來調整畫布大小。

![快速工具列](docs/toolbar.png)
![C2PA 對話框](docs/c2pa-dialog.png)

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
3. 用「打碼框」工具點框，或點清單文字，選取單框（橘框）。按 Delete／Backspace、右鍵或該列刪除按鈕只移除該框；Ctrl+Z 復原、Ctrl+Y 重做。拖曳空白處可新增手動框。清單勾選與關鍵字選取決定哪些框會套用打碼，與橘框選取分開。編輯文字或開啟對話框時，Delete 不會刪框。
4. 選擇打碼方式：**實心色塊（預設，可選顏色）**、馬賽克或模糊，並設定外擴邊距。馬賽克和模糊有時可被部分還原，機密資料請用實心色塊。
5. 套用後會把圖層合併進像素（破壞性、無法從檔案還原）。匯出時預設移除 EXIF／XMP，可在「工具」或「自動打碼」分頁勾選保留。
6. 另有「框選」與「打碼筆刷」可手動打碼。

打碼框是預覽，專案檔不保存這些框；先套用再儲存輸出。刪除預覽框不會移除已套用到圖片的打碼像素。

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
