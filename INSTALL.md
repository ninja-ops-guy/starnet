# Installing AeroTech Staff

AeroTech Staff is a local-first AI workstation. Your data stays on your machine.

## Desktop (Recommended)

Download the latest release for your platform from [aerotech.staff](https://aerotech.staff).

### macOS
1. Open the `.dmg` and drag **AeroTech Staff** into Applications.
2. On first launch, right-click the app and choose **Open** to bypass Gatekeeper.

### Windows
1. Run the `.msi` installer.
2. AeroTech Staff will add itself to Start Menu and can optionally auto-start.

### Linux
1. Extract the `.AppImage` or install the `.deb`/`.rpm`.
2. Make the AppImage executable: `chmod +x AeroTech-Staff-*.AppImage`

## From Source

Requires **Node.js 20+** and **Rust 1.77+**.

```bash
git clone https://github.com/ninja-ops-guy/starnet.git
cd starnet
npm install
npm run tauri dev
```

The first build compiles the Rust shell; subsequent builds are fast.

## Post-Install

On first launch, AeroTech Staff will:
1. Create a workspace directory in `~/.aerotech-staff/`
2. Spawn the Node sidecar on a private loopback port
3. Open the station renderer in a native WebView2 window

No cloud account required. API keys (if you use BYOK providers) are stored in your OS keychain.
