# Rebrand Guide: StarNet → AeroTech Staff

This document describes how to complete the rebrand of a StarNet fork into
AeroTech Staff. The `scripts/rebrand.mjs` script automates the bulk of this;
this guide explains what it does and what still requires manual review.

---

## What the script does (safe replacements)

`npm run rebrand` performs the following **string replacements** across the
codebase, skipping binary files and `node_modules/`:

### User-facing strings (replaced)

| From | To | Where |
|------|-----|-------|
| `StarNet` | `AeroTech Staff` | HTML text, aria-labels, alt text, button labels |
| `STARNET` | `AEROTECH STAFF` | Boot veil, titles, uppercase labels |
| `starnetos.com` | `aerotech.staff` | URLs (placeholder — update when domain registered) |
| `StarNet Remote` | `AeroTech Remote` | Remote feature branding |
| `StarNet Subscription` | `AeroTech Subscription` | Billing UI |
| `StarNet Originals` | `AeroTech Originals` | Skill attribution |

### Code identifiers (NOT replaced — preserve compatibility)

The script deliberately **does NOT** touch:

- JavaScript variable names (`starnetConfig`, `isStarnetManaged`, etc.)
- CSS class names (`.starnet-theme`, etc.)
- localStorage keys (`starnet.*` — see Migration below)
- File paths (`starnet-logo.png` — see Asset Replacement below)
- Internal API routes (`/api/starnet/*`)

These are left as-is to minimize breakage. They can be renamed in a later
cleanup pass once the fork is stable.

---

## What you must do manually

### 1. Brand assets (files)

Replace every file in the table below. See `ASSET-MANIFEST.md` for the
 TechOps Hero sources and generation prompts.

- `frontend/assets/brand/*`
- `src-tauri/icons/*`
- `src-tauri/installer/*`

### 2. Tauri configuration

Edit `src-tauri/tauri.conf.json`:

```json
{
  "productName": "AeroTech Staff",
  "identifier": "io.aerotech.staff",
  "bundle": {
    "publisher": "ninja-ops-guy",
    "icon": [ /* same filenames, new art */ ]
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/ninja-ops-guy/aerotech-staff-releases/releases/latest/download/latest.json"
      ]
    }
  }
}
```

### 3. Updater & releases

Create a new releases repository (`aerotech-staff-releases`) and configure
Tauri's updater to point to it. The updater public key must be regenerated.

### 4. localStorage migration (optional)

If you want to migrate existing users' localStorage from `starnet.*` to
`aerotech.*`, add this to `frontend/app/glass-boot.js` before the first
store read:

```js
// Migrate starnet localStorage keys to aerotech
for (const key of Object.keys(localStorage)) {
  if (key.startsWith('starnet.')) {
    const newKey = key.replace(/^starnet\\./, 'aerotech.');
    localStorage.setItem(newKey, localStorage.getItem(key));
    // localStorage.removeItem(key); // uncomment to clean up
  }
}
```

### 5. Domain & accounts

- Register `aerotech.staff` (or your preferred domain)
- Update all hardcoded URLs in `frontend/index.html` and `frontend/app/*.js`
- Configure OAuth apps (ChatGPT sign-in, Spotify, etc.) with the new domain

### 6. RESIDUAL adapter integration

The `aerotech/` directory contains the RESIDUAL Command Station adapter.
Wire it into the sidecar's provider routing in `sidecar/run.mjs`:

```js
import { ResidualAdapter } from '../aerotech/residual-adapter.mjs';
// Register as a provider alongside OpenRouter, OpenAI, etc.
```

---

## Post-rebrand verification checklist

- [ ] `npm run dev` boots without console errors
- [ ] Boot veil shows "AEROTECH STAFF" (not "STARNET")
- [ ] Window title reads "AeroTech Staff" (not "StarNet")
- [ ] Settings → About shows AeroTech Staff version
- [ ] Asset check passes: `npm run asset-check`
- [ ] Sidecar starts and responds to `/api/status`
- [ ] Agent creation flow works end-to-end
- [ ] Build Mode renders TechOps Hero textures (not StarNet sprites)
- [ ] RESIDUAL adapter appears in provider list
- [ ] Installer art shows AeroTech Staff branding
- [ ] macOS DMG background is correct
- [ ] Windows installer header/sidebar are correct

---

## Rollback

If the rebrand breaks something, the fork relationship with upstream StarNet
is preserved. You can reset to upstream `main` at any time:

```bash
git remote add upstream https://github.com/androoAGI/starnet.git
git fetch upstream
git reset --hard upstream/main
```

All AeroTech Staff changes are on the `aerotech-rebrand` branch.
