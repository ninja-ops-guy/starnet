# AeroTech Staff

> A fork of [StarNet](https://github.com/androoAGI/starnet) — rebuilt with TechOps Hero industrial textures and a top-down day-mode station view.

AeroTech Staff is a **local-first AI workstation** for running agents, managing workflows, and building automated systems. It inherits StarNet's full desktop harness (Tauri shell, Node sidecar, vanilla JS frontend) and re-skins it with TechOps Hero's industrial sci-fi art direction.

---

## What this fork changes

| Layer | StarNet upstream | AeroTech Staff |
|-------|------------------|----------------|
| **Name / brand** | StarNet | AeroTech Staff |
| **Station art** | Top-down pixel-art sprites (Andrew Sims) | TechOps Hero industrial textures — day-mode top-down station view |
| **Characters** | Original sprite roster (Mike, Waldo, Katrin, etc.) | TechOps Hero crew (good boys, ship crews, industrial operators) |
| **Color palette** | Amber/green phosphor CRT | Industrial amber + steel blue + hazard yellow |
| **Backend adapter** | StarNet sidecar only | + RESIDUAL Command Station adapter (`aerotech/`) |

---

## Quick start

```bash
git clone https://github.com/ninja-ops-guy/AeroTech-Staff.git
cd AeroTech-Staff
npm install
npm run dev          # Vite dev server + sidecar
npm run build        # Production desktop build (Tauri)
```

See [docs/INSTALL.md](docs/INSTALL.md) for platform-specific setup (Windows, macOS, Linux).

---

## Architecture

```
┌─────────────────────────────────────────┐
│  Tauri Desktop Shell (src-tauri/)       │
│  - Window management, auto-updater      │
│  - OS keychain for API keys             │
│  - HUD mode, custom titlebar            │
├─────────────────────────────────────────┤
│  Frontend (frontend/)                   │
│  - Vanilla JS, Canvas 2D station view   │
│  - TechOps Hero industrial textures     │
│  - COMMS panel, Crew rail, Build Mode   │
├─────────────────────────────────────────┤
│  Sidecar (sidecar/)                     │
│  - Node.js agent runtime                │
│  - Model routing, tool execution        │
│  - Cron, loops, quest ledger            │
├─────────────────────────────────────────┤
│  AeroTech Adapter (aerotech/)           │
│  - RESIDUAL Command Station integration │
│  - Adversarial reconciliation vectors   │
│  - Qualification-gated module loader    │
└─────────────────────────────────────────┘
```

---

## Asset provenance

All TechOps Hero textures are drawn from the [techops-hero](https://github.com/ninja-ops-guy/techops-hero) asset library. See [`ASSET-MANIFEST.md`](ASSET-MANIFEST.md) for the complete mapping — every replaced file lists its source path, commit SHA, and hash.

StarNet's original code remains under the MIT License (see `STARNET-LICENSE.txt`). This fork's new code and the TechOps Hero textures are provided under the same MIT License.

---

## Contributing

This repo follows the same contribution rules as upstream StarNet, with one addition: **all asset replacements must be documented in `ASSET-MANIFEST.md`** with SHA-256 verification.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow (issue → branch → PR → review → merge).

---

## License

- **StarNet code**: MIT License — Copyright (c) 2026 Andrew Sims. Preserved in `STARNET-LICENSE.txt`.
- **AeroTech Staff changes & TechOps Hero textures**: MIT License — Copyright (c) 2026 ninja-ops-guy.
- **Third-party components**: See [NOTICE.md](NOTICE.md) for skill recipes, fonts, and bundled libraries.

The **AeroTech Staff** name, logo, and station artwork are owned by ninja-ops-guy and are not licensed with the code. Forks must ship their own identity.
