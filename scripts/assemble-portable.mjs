// Builds the portable distribution folder after `tauri build --no-bundle`.
//
//   node scripts/assemble-portable.mjs
//
// Output:
//   release/DSH-Remote-Companion-win-x64/
//     DSHRemoteCompanion.exe
//     Data/README.txt
//     openvr_api.dll        (when SteamVR is installed on the build machine)
//     README.md
//     THIRD_PARTY_NOTICES.md
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const targetDir = path.join(root, 'src-tauri', 'target', 'release');
const distName = 'DSH-Remote-Companion-win-x64';
const distDir = path.join(root, 'release', distName);

const candidates = [
  'DSHRemoteCompanion.exe',
  'dsh-remote-companion.exe',
  'DSH 远控伴侣.exe',
];
let exePath = null;
for (const name of candidates) {
  const candidate = path.join(targetDir, name);
  if (fs.existsSync(candidate)) {
    exePath = candidate;
    break;
  }
}
if (!exePath) {
  console.error(`[assemble] release binary not found in ${targetDir}`);
  console.error('[assemble] run `pnpm build:rust` first');
  process.exit(1);
}

function findSteamVrDll() {
  const local = path.join(path.dirname(exePath), 'openvr_api.dll');
  if (fs.existsSync(local)) return local;

  const localAppData = process.env.LOCALAPPDATA;
  if (localAppData) {
    const vrPath = path.join(localAppData, 'openvr', 'openvrpaths.vrpath');
    try {
      const parsed = JSON.parse(fs.readFileSync(vrPath, 'utf8'));
      for (const runtime of parsed.runtime ?? []) {
        const candidate = path.join(runtime, 'bin', 'win64', 'openvr_api.dll');
        if (fs.existsSync(candidate)) return candidate;
      }
    } catch {
      // SteamVR is optional for packaging; the app also locates it at runtime.
    }
  }

  for (const base of [process.env['ProgramFiles(x86)'], process.env.ProgramFiles]) {
    if (!base) continue;
    const candidate = path.join(
      base,
      'Steam',
      'steamapps',
      'common',
      'SteamVR',
      'bin',
      'win64',
      'openvr_api.dll',
    );
    if (fs.existsSync(candidate)) return candidate;
  }
  return null;
}

fs.rmSync(distDir, { recursive: true, force: true });
fs.mkdirSync(distDir, { recursive: true });
fs.copyFileSync(exePath, path.join(distDir, 'DSHRemoteCompanion.exe'));

const dataDir = path.join(distDir, 'Data');
fs.mkdirSync(dataDir, { recursive: true });
fs.writeFileSync(
  path.join(dataDir, 'README.txt'),
  [
    '此文件夹用于保存设置与缓存。',
    '不要手动删除正在运行时的 Data 文件夹。',
    '移动程序时请把整个文件夹一起复制。',
    '',
  ].join('\r\n'),
  'utf8',
);

const openvrDll = findSteamVrDll();
if (openvrDll) {
  fs.copyFileSync(openvrDll, path.join(distDir, 'openvr_api.dll'));
  console.log(`[assemble] bundled OpenVR runtime: ${openvrDll}`);
} else {
  console.warn('[assemble] openvr_api.dll not found; the app will locate SteamVR at runtime');
}

for (const file of ['README.md', 'THIRD_PARTY_NOTICES.md', 'LICENSE']) {
  const source = path.join(root, file);
  if (fs.existsSync(source)) fs.copyFileSync(source, path.join(distDir, file));
}

const zipPath = path.join(root, 'release', `${distName}.zip`);
fs.rmSync(zipPath, { force: true });
try {
  execFileSync(
    'powershell',
    [
      '-NoProfile',
      '-Command',
      `Compress-Archive -LiteralPath '${distDir.replaceAll("'", "''")}' -DestinationPath '${zipPath.replaceAll("'", "''")}' -Force`,
    ],
    { stdio: 'inherit' },
  );
} catch {
  console.warn('[assemble] Compress-Archive failed; the portable folder is still ready');
}

console.log(`[assemble] ${distName} -> ${distDir}`);
console.log(`[assemble] binary: ${path.join(distDir, 'DSHRemoteCompanion.exe')}`);
if (fs.existsSync(zipPath)) console.log(`[assemble] zip: ${zipPath}`);
