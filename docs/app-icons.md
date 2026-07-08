# Pixel Studio App Icons

## Source

The icon pack was generated with:

```text
D:\ico_create\skills\codex-app-icon-generator
```

Master artwork:

```text
assets/icon/master.png
```

The master icon is a square Pixel Studio symbol: a pixel-grid canvas with a brush stroke, using blue/cyan contrast and no text.

## Generated Pack

The generated cross-platform icon pack lives in:

```text
assets/icon/pack/
```

Important outputs:

- Windows ICO: `assets/icon/pack/windows/pixel-studio.ico`
- Runtime PNG icon: `assets/icon/pack/png/256x256.png`
- Linux hicolor PNGs: `assets/icon/pack/linux/hicolor/`
- Web/PWA favicons and manifest: `assets/icon/pack/web/`
- macOS iconset fallback: `assets/icon/pack/macos/pixel-studio.iconset/`
- Report: `assets/icon/pack/icon-pack-report.json`

## Wiring

- `crates/studio-app/src/icons.rs` embeds the 256px PNG for the runtime window icon.
- `crates/studio-app/build.rs` embeds the Windows `.ico` into the release executable with `winresource`.

## Validation

- `build_icon_pack.py --check-only` completed successfully.
- The Windows `.ico` contains 16, 24, 32, 48, 64, 128, and 256 px entries.
- A release `studio-app.exe` associated icon extraction returned a 32x32 app icon matching the generated pack.
