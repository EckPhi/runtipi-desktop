import { readFileSync } from 'node:fs';
const packageVersion = JSON.parse(readFileSync('package.json', 'utf8')).version;
const tauriConfig = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const tauriVersion = tauriConfig.version;
const cargoVersion = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
const tag = process.argv[2];
if (new Set([packageVersion, tauriVersion, cargoVersion]).size !== 1 || tag !== `v${packageVersion}`) {
  console.error(`Version mismatch: tag=${tag}, npm=${packageVersion}, Tauri=${tauriVersion}, Cargo=${cargoVersion}`);
  process.exit(1);
}
console.log(`Release versions match: ${tag}`);
const msiVersion = tauriConfig.bundle?.windows?.wix?.version;
const parts = msiVersion?.split('.').map(Number);
if (!parts || !/^\d+\.\d+\.\d+(?:\.\d+)?$/.test(msiVersion) ||
    parts.slice(0, 3).join('.') !== packageVersion.split('-')[0] ||
    parts.some((value, index) => value > (index < 2 ? 255 : 65535))) {
  console.error(`MSI version must be numeric and match the app's major.minor.patch: ${msiVersion}`);
  process.exit(1);
}
console.log(`Windows installer version is valid: ${msiVersion}`);
