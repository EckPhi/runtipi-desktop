import { readFileSync } from 'node:fs';
const packageVersion = JSON.parse(readFileSync('package.json', 'utf8')).version;
const tauriVersion = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
const cargoVersion = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
const tag = process.argv[2];
if (new Set([packageVersion, tauriVersion, cargoVersion]).size !== 1 || tag !== `v${packageVersion}`) {
  console.error(`Version mismatch: tag=${tag}, npm=${packageVersion}, Tauri=${tauriVersion}, Cargo=${cargoVersion}`);
  process.exit(1);
}
console.log(`Release versions match: ${tag}`);
