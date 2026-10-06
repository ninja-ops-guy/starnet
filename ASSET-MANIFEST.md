# AeroTech Staff Asset Manifest

> Every texture, sprite, and brand asset replaced in this fork is listed here with its
> source path, upstream commit, and SHA-256 hash. This file is normative — no asset may
> be committed to `frontend/assets/` without an entry here.

## Asset replacement strategy

StarNet's renderer is **top-down 2D** (Canvas 2D, tile-based). TechOps Hero's **day mode**
provides overhead/industrial views that map naturally to this perspective. The replacement
follows these rules:

1. **Floor / terrain tiles** → TechOps Hero campaign backgrounds and visual-combat environments
2. **Wall / machinery tiles** → TechOps Hero industrial textures and plating line backgrounds
3. **Character sprites** → TechOps Hero crew atlases (top-down cropped where needed)
4. **Brand / UI** → Custom AeroTech Staff identity (logo, wordmark, installer art)
5. **Sound effects** → TechOps Hero audio where available, otherwise preserved from upstream

---

## Tier 1: Environment & Station (priority — affects every screen)

| StarNet path | TechOps Hero source | Source commit | SHA-256 | Status |
|-------------|---------------------|---------------|---------|--------|
| `frontend/assets/industrial/floor-*.png` | `assets/campaign/plating.line_background.png` | `techops-hero@HEAD` | `TBD` | 🔲 pending |
| `frontend/assets/industrial/wall-*.png` | `assets/campaign/shipping.dock_background.png` | `techops-hero@HEAD` | `TBD` | 🔲 pending |
| `frontend/assets/brand/starnet-logo.png` | *(generated)* — see `docs/brand-guidelines.md` | — | `TBD` | 🔲 pending |
| `frontend/assets/brand/starnet-wordmark.svg` | *(generated)* — traced from new logo | — | `TBD` | 🔲 pending |
| `src-tauri/icons/32x32.png` | *(generated)* — AeroTech Staff mark | — | `TBD` | 🔲 pending |
| `src-tauri/icons/128x128.png` | *(generated)* — AeroTech Staff mark | — | `TBD` | 🔲 pending |
| `src-tauri/icons/128x128@2x.png` | *(generated)* — AeroTech Staff mark | — | `TBD` | 🔲 pending |
| `src-tauri/icons/icon.icns` | *(generated)* — AeroTech Staff mark | — | `TBD` | 🔲 pending |
| `src-tauri/icons/icon.ico` | *(generated)* — AeroTech Staff mark | — | `TBD` | 🔲 pending |
| `src-tauri/installer/header.bmp` | *(generated)* — industrial header | — | `TBD` | 🔲 pending |
| `src-tauri/installer/sidebar.bmp` | *(generated)* — industrial sidebar | — | `TBD` | 🔲 pending |
| `src-tauri/installer/dmg-background.png` | *(generated)* — industrial DMG bg | — | `TBD` | 🔲 pending |

## Tier 2: Characters & Props (priority — affects station population)

| StarNet path | TechOps Hero source | Source commit | SHA-256 | Status |
|-------------|---------------------|---------------|---------|--------|
| `frontend/assets/sprites/approved_android/` | `assets/v736/k_action_atlas.png` (cropped) | `techops-hero@HEAD` | `TBD` | 🔲 pending |
| `frontend/assets/sprites/approved_bot/` | `assets/v736/k_studio_atlas.png` (cropped) | `techops-hero@HEAD` | `TBD` | 🔲 pending |
| `frontend/assets/sprites/approved_frog/` | `assets/v736/k_action_atlas.png` (cropped) | `techops-hero@HEAD` | `TBD` | 🔲 pending |
| `frontend/assets/sprites/approved_warrior/` | `assets/v742/k_action_atlas.png` (cropped) | `techops-hero@HEAD` | `TBD` | 🔲 pending |
| `frontend/assets/sprites/approved_human/` | `assets/good_boys/` (top-down crop) | `techops-hero@HEAD` | `TBD` | 🔲 pending |
| `frontend/assets/furniture/*.png` | `assets/visual-combat/home.webp` (tile extract) | `techops-hero@HEAD` | `TBD` | 🔲 pending |

## Tier 3: Effects & Audio (lower priority — preserved from upstream if no replacement)

| StarNet path | TechOps Hero source | Source commit | SHA-256 | Status |
|-------------|---------------------|---------------|---------|--------|
| `frontend/assets/sfx/*.ogg` | `assets/audio/*.ogg` (where available) | `techops-hero@HEAD` | `TBD` | 🔲 pending |
| `frontend/assets/fonts/vt323.woff2` | *(preserved)* — VT323 is part of the CRT aesthetic | `starnet@fbddbf9` | `TBD` | ✅ preserved |

---

## Generation spec (AI-generated assets)

Assets marked *(generated)* above are produced with the following prompt template:

> "Top-down pixel-art industrial sci-fi station tile in the style of TechOps Hero.
> Muted steel palette with amber hazard markings. 32x32 tile, seamless, transparent
> background where appropriate. Day mode lighting — bright, functional, no neon."

All generated assets are committed with a `.gen.md` sidecar file documenting the exact
prompt, seed, and model version used.

---

## Verification

Run `npm run asset-check` to verify every manifest entry against the live tree:
- File exists at the declared path
- SHA-256 matches the declared hash
- Source commit is reachable in the techops-hero repo

This check runs in CI on every PR that touches `frontend/assets/`.
