# Runtipi Desktop

A desktop workspace for multiple [Runtipi](https://runtipi.io) instances, built with Rust, Tauri 2, Svelte 5, and TypeScript. An independent project, not an official Runtipi client.

- Add, edit, remove, reorder, and choose a default instance.
- Switch between instance workspaces with a permanent Dashboard tab.
- Open application URLs in tabs; dashboard popup links request new tabs.
- Back, forward, reload, and open in your external browser.
- Persistent, separate browser storage for each instance.

## Downloads

Installers are published to [GitHub Releases](https://github.com/EckPhi/runtipi-desktop/releases). Windows x64, Linux x64 (Debian/RPM/AppImage), and macOS Intel / Apple Silicon are targeted. macOS 14+ is required for separate persistent browser profiles.

Initial releases are unsigned and not notarized. Tab/window rendering and app authentication still need manual verification on each operating system. Runtipi and its apps must be reachable from your desktop; this client does not provide a VPN or tunnel.

## Development

Install Node.js 24, stable Rust, and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
npm ci
npm run tauri dev
```

On Ubuntu/Debian, the build requires `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, and `patchelf`.

```sh
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run tauri -- build -- --locked
```

## Architecture and storage

The local Svelte shell owns settings and tab controls. Rust persists instance configuration as `instances.json` in the OS application config directory and creates a native child webview per tab. Remote content has no application capabilities; privileged commands also check the calling webview label. HTTP and HTTPS addresses are supported; URLs containing credentials are rejected. Sign in directly through the embedded dashboard or app.

Windows/Linux profiles live in the OS application data directory under `profiles/<instance UUID>`; macOS uses the corresponding WebKit data-store UUID. Removing an instance closes its tabs but retains browser data. Tabs and the active selection are kept during a run; the configured default instance opens on restart. Browser storage is managed by the native engine, not an app password store.

Tauri's child-webview API currently requires its `unstable` Cargo feature. Popup requests open fresh tabs; opener relationships are not preserved, so OAuth or applications relying on `window.opener` may require the external browser. Native engine behaviour differs across operating systems. Downloads, media/device permissions, pinned shortcuts, session restoration, profile deletion, API-based app discovery, and signed automatic updates are follow-up work.

## CI and releases

`.github/workflows/desktop.yml` validates and tests the frontend, formats/lints/tests Rust, and builds installers on all four targets for pushes, pull requests, and manual runs. Installers are retained as workflow artifacts. A `v*` tag additionally publishes a GitHub Release only after all builds succeed. Versions must match across `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`. Windows MSI requires a numeric version, so `bundle.windows.wix.version` explicitly uses `major.minor.patch.build` (for example, `0.1.0.2` for `0.1.0-alpha.2`); update it alongside the app version. The release guard verifies its numeric format and matching base version.

To release, update all three version fields and the npm/Cargo lockfiles, commit, and push a matching tag:

```sh
git tag v0.2.0
git push origin v0.2.0
```

Tags containing a hyphen (such as `v0.1.0-alpha.1`) produce prereleases. No deployment secrets are needed: the release job uses the repository-scoped GitHub Actions token. Code signing, notarization, and an in-app updater are not configured yet.

## License

MIT. See [LICENSE](LICENSE).
