# Gosling Logo Asset Pack

Generated logo assets for **gosling**, a lightweight fork-style identity inspired by the simple monochrome bird-mark direction.

## Included

- `source/` — editable SVG source files, including tight and square versions.
- `png/` — transparent, white-background, and black-background PNG exports from 16px to 1024px.
- `web/` — favicon PNGs, `favicon.svg`, Apple touch icon, PWA icons, and `manifest.webmanifest`.
- `windows/` — multi-resolution `.ico` plus PNG fallbacks.
- `macos/` — `.icns` file and `.iconset` source folder.
- `linux/` — hicolor icon theme structure, scalable SVG, and a starter `.desktop` file.
- `ios/` — Xcode-ready `AppIcon.appiconset` with `Contents.json`.
- `android/` — Android density folders plus adaptive icon XML starter files.
- `buttons/` — SVG and PNG button/badge variants.

## Notes

- iOS app icons are opaque white-background PNGs, as required by Apple app icon conventions.
- Android adaptive-icon XML is included as a starter; depending on your build setup, you may want to tune foreground safe-zone scaling.
- For web use, copy `web/favicon.svg`, `web/favicon-32x32.png`, `web/apple-touch-icon.png`, and `web/manifest.webmanifest` into your public/static folder.
