# Pixel Studio

English ｜ [繁體中文](README.md)

A Rust + egui image editor that aims to **replace and surpass MS Paint**: painting, layers, stickers, social-media post images, offline auto-redaction and batch conversion, on Windows, Linux and macOS.

Downloads: [Releases](https://github.com/stevenke1981/pixel-studio/releases)
- Windows: `pixel-studio-setup-windows-x64.exe` (installer) or `pixel-studio-windows-x64.zip` (portable)
- Linux: `pixel-studio-linux-x64.tar.gz`
- macOS (Intel + Apple silicon): `pixel-studio-macos-universal-app.zip`

## Compared with Paint

| Feature | Paint | Pixel Studio |
|---|---|---|
| Pencil, brush, eraser, fill, colour picker, text | ✓ | ✓ (plus highlighter, airbrush, calligraphy, mosaic) |
| Shapes: line, arrow, rectangle, ellipse, triangle | ✓ | ✓ (colour 1 outline, optional colour 2 fill; shapes stay editable layers) |
| Select, crop, resize, rotate, flip | ✓ | ✓ (rectangle/circle selection, crop to selection, drag canvas edges) |
| Zoom, undo/redo | ✓ | ✓ (crisp pixels when zoomed + pixel grid, Ctrl+G) |
| Layers, transparency | partial | ✓ text / image / shape / sticker layers: move, reorder, hide, opacity |
| Brightness, contrast, saturation, hue, blur, sharpen | ✗ | ✓ plus exposure, temperature, tint, sepia, vignette, posterize, auto contrast, grayscale, invert |
| Sticker library (arrows, callouts, hearts/stars, checks/crosses, numbers, highlight boxes, emoji, NEW/HOT/SALE labels, generic social icons) | ✗ | ✓ 65 original SVGs: search, click or drag to insert, resize, recolour, outline, shadow |
| Social canvas presets | ✗ | ✓ IG post 1080×1080, IG portrait 1080×1350, IG story 1080×1920, FB 1200×630, X 1600×900, YouTube thumbnail 1280×720, LINE 1040×1040 |
| Quick text styles (outlined / shadowed titles) | ✗ | ✓ |
| Auto redaction | ✗ | ✓ offline text detection, pre-selects phones, emails, TW IDs, cards, URLs, addresses |
| Paste clipboard as a new image, screenshots | partial | ✓ |
| Formats | PNG/JPEG/BMP/GIF/TIFF/HEIC | PNG, JPEG, BMP, GIF, ICO, WebP (lossless + lossy with quality), TIFF |
| Batch convert, animated GIF, 360° panorama, C2PA | ✗ | ✓ |
| Right-click menu, Open With | built in | ✓ "Edit / Open with Pixel Studio", never hijacks your default app |

## Auto Redact (▦ Auto Redact tab)

1. Click **Detect text**. Text is detected locally with the [ocrs](https://github.com/robertknight/ocrs) models (bundled in the installer and archives, ~12 MB; downloaded once on first use when running from source).
2. Boxes are drawn over the image. Lines recognised as phone numbers (TW + international), emails, Taiwan national IDs (checksum), credit cards (Luhn), URLs or addresses are pre-selected (red).
3. Click a box to toggle, right-click to delete, drag to add, edit coordinates in the list, or select by keyword.
4. Pick a style: **solid box (default, any colour)**, mosaic or blur, plus padding. Mosaic and blur can sometimes be partially reversed; use a solid box for secrets.
5. Applying flattens layers into the pixels (destructive). Exports strip EXIF/XMP by default; tick "Keep EXIF" in Properties to keep it.
6. Manual **Boxes** and **Redact brush** tools are included.

**Limitation:** detection is script-agnostic, so Chinese and English lines are both boxed, but *recognition* currently reads Latin letters, digits and symbols only. Chinese-only content (for example a Chinese street address) is not auto-selected and must be clicked manually.

CLI: `studio-cli redact in.png out.png [--style solid|mosaic|blur] [--keyword WORD] [--all] [--keep-metadata]`

## Shell menu and file associations

- Installer tasks: right-click menu, add to "Open with" (on by default, keeps your default app), make default for PNG/WebP (off by default), desktop icon.
- Portable: `pixel-studio --register-shell` (current user only; HKCU on Windows), undo with `--unregister-shell`.
- Linux: `.desktop` (Open With), Nautilus scripts, Dolphin service menu, Nemo actions. macOS: Finder Quick Actions, and the `.app` declares image document types.
- Arguments: `pixel-studio [--edit|--view] FILE`

## Languages

Traditional Chinese (default on Chinese systems) and English (default elsewhere). Switch in the Language menu or set `PIXEL_STUDIO_LANG=en`.

## Building

```bash
cargo build --release          # target/release/pixel-studio and studio-cli
./scripts/fetch-models.sh      # optional: OCR models into models/
cargo test --workspace
```

## Icon licence

All stickers and the app icon are original artwork for this project (MIT). **No platform trademarks** (Facebook, Instagram, X, LINE, YouTube…) are included; the social category uses generic icons (like, share, comment, play, bell, camera, mention, hashtag, location).

## License

MIT
