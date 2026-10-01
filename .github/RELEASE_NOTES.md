Desktop installers for Windows x64, Linux x64, and macOS Intel / Apple Silicon (macOS 14+).

Windows builds are unsigned. macOS app bundles are ad-hoc signed and their signatures are verified inside each DMG in CI, but the apps are not notarized. Copy the app to Applications, eject the disk image, and use Privacy & Security → Open Anyway if offered. If Gatekeeper reports the app as damaged, follow the app-specific quarantine instructions in the README. Automatic in-app updates are not enabled. Download the installer matching your device.

This is an early desktop shell for your existing Runtipi dashboards. Add an instance in Settings, sign in through its dashboard, and open apps in tabs. Browser sessions are persisted per instance; open tabs are kept during the current run only. Popup-dependent authentication and individual apps need testing on your operating system. Use “Open externally” for apps that require your regular browser.

Proton Pass filling: install the official Proton Pass CLI, run `pass-cli login`, and configure a vault/path in Settings if needed. Choose **Proton Pass** on a login page, then select a saved login. Its URL must match the current scheme, hostname, and port. Supports username/password fields and separate login steps; does not auto-submit. Passkeys, TOTP, iframe/shadow-DOM forms, and ambiguous forms are not supported yet. Bitwarden has no direct adapter yet; use desktop drag-and-drop where compatible or your regular browser extension through Open externally.

Includes the macOS browser/header overlap fix. Real-vault authentication and filling still need verification on your device.
