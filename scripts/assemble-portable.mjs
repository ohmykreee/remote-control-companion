// Builds the portable distribution folder after `tauri build --no-bundle`
// (or `cargo build --release`).
//
//   node scripts/assemble-portable.mjs [--target <rust-triple>]
//
// Output:
//   release/DSH-Remote-Companion-<platform>-<arch>/
//     DSHRemoteCompanion[.exe]
//     Data/README.txt
//     openvr_api.dll        (Windows only, when SteamVR is installed)
//     README.md
//     THIRD_PARTY_NOTICES.md
//     LICENSE
//
// The target triple is explicit in CI, so a cross build is never labelled as
// the host platform. Without --target the host's own release directory is used.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const args = process.argv.slice(2);
const targetIndex = args.indexOf('--target');
if (targetIndex >= 0 && !args[targetIndex + 1]) {
  console.error('[assemble] --target needs a Rust target triple');
  process.exit(1);
}
const target = targetIndex >= 0 ? args[targetIndex + 1] : null;

function hostTriple() {
  const os =
    process.platform === 'win32'
      ? 'pc-windows-msvc'
      : process.platform === 'darwin'
        ? 'apple-darwin'
        : 'unknown-linux-gnu';
  const arch = process.arch === 'x64' ? 'x86_64' : process.arch === 'arm64' ? 'aarch64' : process.arch;
  return `${arch}-${os}`;
}

function describe(triple) {
  const platform = triple.includes('windows')
    ? 'win'
    : triple.includes('apple-darwin')
      ? 'mac'
      : triple.includes('linux')
        ? 'linux'
        : null;
  if (!platform) throw new Error(`[assemble] cannot tell the platform from "${triple}"`);
  const arch = triple.startsWith('x86_64')
    ? 'x64'
    : triple.startsWith('aarch64')
      ? 'arm64'
      : null;
  if (!arch) throw new Error(`[assemble] cannot tell the architecture from "${triple}"`);
  return { platform, arch, label: `${platform}-${arch}` };
}

const triple = target ?? hostTriple();
const { platform, label } = describe(triple);
const distName = `DSH-Remote-Companion-${label}`;
const targetDir = target
  ? path.join(root, 'src-tauri', 'target', target, 'release')
  : path.join(root, 'src-tauri', 'target', 'release');
const distDir = path.join(root, 'release', distName);
const extension = platform === 'win' ? '.exe' : '';
const shippedName = `DSHRemoteCompanion${extension}`;
const candidates = [
  `DSHRemoteCompanion${extension}`,
  `dsh-remote-companion${extension}`,
  `DSH 远控伴侣${extension}`,
];

let binary = null;
const existing = candidates
  .map((name) => path.join(targetDir, name))
  .filter((candidate) => fs.existsSync(candidate))
  .map((candidate) => ({ candidate, mtime: fs.statSync(candidate).mtimeMs }))
  .sort((a, b) => b.mtime - a.mtime);
if (existing.length > 0) {
  binary = existing[0].candidate;
}
if (!binary) {
  console.error(`[assemble] release binary not found in ${targetDir}`);
  console.error(`[assemble] looked for: ${candidates.join(', ')}`);
  process.exit(1);
}

function findSteamVrDll() {
  if (platform !== 'win') return null;
  const local = path.join(path.dirname(binary), 'openvr_api.dll');
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
fs.copyFileSync(binary, path.join(distDir, shippedName));

const dataDir = path.join(distDir, 'Data');
fs.mkdirSync(dataDir, { recursive: true });
fs.writeFileSync(
  path.join(dataDir, 'README.txt'),
  [
    '此文件夹用于保存设置与缓存。',
    '不要手动删除正在运行时的 Data 文件夹。',
    '移动程序时请把整个文件夹一起复制。',
    '',
  ].join(platform === 'win' ? '\r\n' : '\n'),
  'utf8',
);

const openvrDll = findSteamVrDll();
if (openvrDll) {
  fs.copyFileSync(openvrDll, path.join(distDir, 'openvr_api.dll'));
  console.log(`[assemble] bundled OpenVR runtime: ${openvrDll}`);
} else if (platform === 'win') {
  console.warn('[assemble] openvr_api.dll not found; the app will locate SteamVR at runtime');
}

for (const file of ['README.md', 'THIRD_PARTY_NOTICES.md', 'LICENSE']) {
  const source = path.join(root, file);
  if (fs.existsSync(source)) fs.copyFileSync(source, path.join(distDir, file));
}

const zipPath = path.join(root, 'release', `${distName}.zip`);
fs.rmSync(zipPath, { force: true });
try {
  if (platform === 'win') {
    // Use Windows' bundled bsdtar explicitly. GitHub Actions runs this script
    // from Git Bash, where plain `tar` resolves to GNU tar, which does not
    // understand bsdtar's `-a` auto-archive flag.
    const windowsTar = path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe');
    if (!fs.existsSync(windowsTar)) {
      throw new Error(`Windows tar not found at ${windowsTar}`);
    }
    execFileSync(
      windowsTar,
      ['-a', '-c', '-f', zipPath, '-C', path.dirname(distDir), distName],
      { stdio: 'inherit' },
    );
  } else {
    execFileSync('zip', ['-r', '-q', zipPath, distName], {
      cwd: path.dirname(distDir),
      stdio: 'inherit',
    });
  }
} catch (error) {
  console.error(`[assemble] archive failed: ${error.message}`);
  process.exit(1);
}
if (!fs.existsSync(zipPath)) {
  console.error(`[assemble] archive was not created: ${zipPath}`);
  process.exit(1);
}

console.log(`[assemble] ${distName} -> ${distDir}`);
console.log(`[assemble] binary: ${path.join(distDir, shippedName)}`);
if (fs.existsSync(zipPath)) console.log(`[assemble] zip: ${zipPath}`);
