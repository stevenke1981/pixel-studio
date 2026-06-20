# 圖片影像編輯軟體 — UI 設計規格書

> 版本：v1.0
> 語言：zh-TW（技術名詞保留 English）
> 用途：供 AI coding agent（build agent）實作 UI 原型時的設計依據
> 對應原型檔：`ui_prototype.html`（同目錄，可直接於瀏覽器開啟檢視）

---

## 1. 概觀

- **產品定位**：桌面端影像編輯軟體（類 Photoshop / Affinity Photo）
- **設計哲學**：扁平化（flat）、無漸層／無陰影、細邊框（0.5px）、緊湊留白
- **建議技術棧**：React + TypeScript + CSS variables（淺／深色模式自動切換）
- **最小視窗寬度建議**：1024px（行動裝置需另行設計響應式版本）

---

## 2. 版面結構 Layout

整體為上下四段式，主工作區再分左中右三欄：

| 區塊 | 尺寸 | 說明 |
|---|---|---|
| 頂部選單列 `TopMenuBar` | height: 40px | App 名稱、選單項目（檔案/編輯/影像/圖層/濾鏡/檢視）、復原/重做、匯出按鈕 |
| 情境工具列 `ContextualToolbar` | height: 34px | 隨目前選取工具動態切換的參數列 |
| 主工作區 `Workspace` | flex: 1，建議 min-height: 480px | 左中右三欄，見下表 |
| 狀態列 `StatusBar` | height: 26px | 影像尺寸、色彩模式、縮放控制 |

主工作區三欄：

| 欄位 | 寬度 | 內容 |
|---|---|---|
| 工具軸 `ToolRail` | 50px 固定 | 垂直排列工具圖示，含選取態 |
| 畫布 `Canvas` | flex: 1, min-width: 0 | 顯示影像、選取框、縮放百分比浮動標籤 |
| 屬性面板 `RightPanel` | 260px 固定 | 分頁：圖層／調整／紀錄 |

`min-width: 0` 為必要設定，避免 flex 子元素內容（如過長圖層名稱）撐破版面。

---

## 3. 設計 Token

### 3.1 顏色（語意化變數，需同時支援淺／深色模式）

| Token | 用途 | 淺色範例值 |
|---|---|---|
| `--bg-primary` | 主背景（面板、選單列） | `#ffffff` |
| `--bg-secondary` | 次背景（工具軸、狀態列） | `#f5f5f3` |
| `--bg-tertiary` | 畫布外圍背景 | `#ececea` |
| `--bg-info` | 選取態背景（工具/分頁 active） | `#E6F1FB` |
| `--text-primary` | 主要文字 | `#1a1a1a` |
| `--text-secondary` | 次要文字（標籤、說明） | `#6b6b66` |
| `--text-tertiary` | 輔助文字（提示、鎖定圖示） | `#9a9893` |
| `--text-info` | 選取態文字／連結 | `#0C447C` |
| `--border-tertiary` | 預設邊框（0.5px） | `rgba(0,0,0,.08)` |
| `--border-secondary` | hover 邊框 | `rgba(0,0,0,.15)` |
| `--border-info` | 強調邊框（分頁底線、選取框） | `#378ADD` |
| `--radius-md` | 一般圓角 | `8px` |
| `--radius-lg` | 卡片／視窗圓角 | `12px` |

完整 token 定義見附錄程式碼 `:root` 區塊。

### 3.2 字體規則

- 主要字體：系統 sans-serif
- 字級：選單/標籤 11–12px，內文 13px，無小於 11px
- 字重：僅使用 400（一般）與 500（強調），不使用 600/700
- 一律使用句子大小寫，不使用全大寫或 Title Case

---

## 4. 元件規格

### 4.1 工具軸 `ToolRail`

工具清單與對應 icon（[Tabler Icons](https://tabler.io/icons)）：

| 工具 ID | 顯示名稱 | icon class | 建議快捷鍵 |
|---|---|---|---|
| `move` | 移動 | `ti-pointer` | V |
| `lasso` | 套索 | `ti-lasso` | L |
| `crop` | 裁切 | `ti-crop` | C |
| `brush` | 筆刷 | `ti-brush` | B |
| `eraser` | 橡皮擦 | `ti-eraser` | E |
| `stamp` | 仿製印章 | `ti-stamp` | S |
| `text` | 文字 | `ti-typography` | T |
| `shape` | 圖形 | `ti-square` | U |
| `eyedropper` | 滴管 | `ti-color-picker` | I |
| `zoom` | 縮放 | `ti-zoom-in` | Z |

狀態樣式：

```css
/* 未選取 */
.tool-btn { background: transparent; color: var(--text-secondary); }
/* hover */
.tool-btn:hover { background: var(--bg-secondary); }
/* 選取中 */
.tool-btn[data-active="1"] { background: var(--bg-info); color: var(--text-info); }
```

### 4.2 情境工具列 `ContextualToolbar`

依目前工具動態渲染參數，預設（筆刷工具）包含：

| 參數 | 型別 | 範圍 | 預設值 |
|---|---|---|---|
| 筆刷大小 | range | 1–200px | 60px |
| 硬度 | range | 0–100% | 80% |
| 不透明度 | range | 0–100% | 100% |
| 混合模式 | select | 正常/色彩增值/濾色... | 正常 |

### 4.3 畫布 `Canvas`

- 背景使用 `--bg-tertiary`，置中顯示影像
- 選取範圍以 1px dashed `--border-info` 外框表示，不可有圓角（單純矩形選取框）
- 右下角浮動顯示目前縮放百分比（小型 badge，`--bg-primary` 背景 + `--border-tertiary` 邊框）

### 4.4 屬性面板 `RightPanel`

三分頁，**同一時間僅渲染一個 panel 的內容**，其餘以 `display:none` 隱藏；切換邏輯為簡單的 active class 切換，不需要路由。

**圖層分頁 `LayersPanel`**
- 每列：可見性 toggle（`ti-eye`）、縮圖 26×26px、圖層名稱、鎖定圖示（背景層）
- 選取後底部顯示「不透明度」滑桿（0–100%）

**調整分頁 `AdjustPanel`**
- 四組 range slider：亮度、對比、飽和度、色溫，皆為 0–100，預設 50
- 數值顯示需即時更新並四捨五入為整數（避免浮點數誤差如 `49.00000001`）

**紀錄分頁 `HistoryPanel`**
- 操作歷史清單，最新一筆（目前狀態）以 `--text-info` 標示並加粗（font-weight: 500）

### 4.5 狀態列 `StatusBar`

- 左側：`{寬} × {高} px・{色彩模式}・{位元深度}`，例：`1920 × 1080 px・RGB・8bit`
- 右側：縮小/放大圖示 + 縮放滑桿（10–400%，step 10）+ 百分比數值

---

## 5. 互動狀態定義

| 元件 | 狀態 | 視覺變化 |
|---|---|---|
| 工具按鈕 | hover | `background: var(--bg-secondary)` |
| 工具按鈕 | 選取中 | `background: var(--bg-info); color: var(--text-info)` |
| 分頁按鈕 | active | `border-bottom: 2px solid var(--border-info)` |
| Range slider | input 事件 | 即時更新對應數值文字，並四捨五入 |
| 圖層列 | hover | `background: var(--bg-secondary)` |

---

## 6. 建議元件樹（React，給 build agent 實作參考）

```
<ImageEditorApp>
 ├─ <TopMenuBar />
 ├─ <ContextualToolbar tool={activeTool} />
 ├─ <Workspace>
 │   ├─ <ToolRail tools={TOOL_LIST} active={activeTool} onSelect={setActiveTool} />
 │   ├─ <Canvas image={currentImage} selection={selectionRect} zoom={zoom} />
 │   └─ <RightPanel>
 │        ├─ <PanelTabs tabs={['layers','adjust','history']} active={tab} onChange={setTab} />
 │        ├─ {tab === 'layers'  && <LayersPanel layers={layers} onToggleVisible={...} onOpacityChange={...} />}
 │        ├─ {tab === 'adjust'  && <AdjustPanel values={adjustValues} onChange={setAdjustValues} />}
 │        └─ {tab === 'history' && <HistoryPanel entries={historyEntries} />}
 │      </RightPanel>
 └─ <StatusBar dimensions={imgDims} zoom={zoom} onZoomChange={setZoom} />
```

### TypeScript 型別建議

```ts
// 工具列舉
type ToolId =
  | "move" | "lasso" | "crop" | "brush" | "eraser"
  | "stamp" | "text" | "shape" | "eyedropper" | "zoom";

// 圖層資料結構
interface Layer {
  id: string;
  name: string;
  thumbnailColor: string;   // 縮圖代表色，正式版應為實際縮圖 URL
  visible: boolean;
  locked?: boolean;
  opacity: number;          // 0-100
  blendMode: "normal" | "multiply" | "screen";
}

// 調整面板數值，皆為 0-100，預設 50（中間值代表「未調整」）
interface AdjustValues {
  brightness: number;
  contrast: number;
  saturation: number;
  temperature: number;
}

interface HistoryEntry {
  id: string;
  label: string;
  isCurrent: boolean;
}
```

---

## 7. 無障礙 Accessibility 注意事項

- 所有 icon-only 按鈕必須加上 `aria-label`
- 分頁與工具切換需可用鍵盤操作（Tab 移動 + Enter/Space 觸發）
- 文字疊加於有色背景時，一律使用該色階的 800/900 階文字色，禁止直接疊加純黑色
- 滑桿（range input）需可用方向鍵微調

---

## 8. 已知限制 / 待擴充項目

- 目前規格未涵蓋：多視窗／多文件分頁、外掛系統、快速鍵自訂面板
- 行動裝置版本需重新設計（建議改為底部抽屜式工具列，而非左側固定軸）
- 圖層縮圖目前以純色色塊代表，正式實作應改為真實影像縮圖（建議 lazy render + canvas downscale）

---

## 附錄：參考實作

完整可執行的靜態原型請見同目錄 `ui_prototype.html`，可直接以瀏覽器開啟，包含：

- 完整 HTML 結構（對應本文件第 2、4 節）
- CSS 變數定義（對應第 3.1 節 token 表）
- 基本互動 JS（工具切換、分頁切換、滑桿數值綁定）

agent 實作時建議流程：先讀本文件建立元件樹與 props 介面 → 對照 `ui_prototype.html` 還原視覺細節 → 以實際框架（React/Vue 等）重新實作，不需逐字搬移原型中的 inline style。
