// Stages a sofalizer(libmysofa)-capable ffmpeg binary into bin/ for local development.
// Windows and macOS-Intel have confirmed prebuilt sources; other platforms print
// instructions instead of guessing at an unverified download (see the plan's
// platform-support matrix). The bot still runs everywhere without this step —
// it just falls back to the lightweight (non-HRTF) spatial audio chain.
import { mkdir, rm, copyFile, readdir } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..');
const BIN_DIR = path.join(PROJECT_ROOT, 'bin');
const TMP_DIR = path.join(PROJECT_ROOT, '.ffmpeg-setup-tmp');

async function download(url, destPath) {
  console.log(`Downloading ${url} ...`);
  const res = await fetch(url);
  if (!res.ok || !res.body) {
    throw new Error(`Failed to download ${url}: ${res.status} ${res.statusText}`);
  }
  await pipeline(Readable.fromWeb(res.body), createWriteStream(destPath));
}

async function setupWindows() {
  const archiveUrl = 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full.7z';
  const archivePath = path.join(TMP_DIR, 'ffmpeg-full.7z');
  await mkdir(TMP_DIR, { recursive: true });
  await download(archiveUrl, archivePath);

  const { default: sevenZip } = await import('7zip-min');
  await new Promise((resolve, reject) => {
    sevenZip.unpack(archivePath, TMP_DIR, (err) => (err ? reject(err) : resolve()));
  });

  const entries = await readdir(TMP_DIR);
  const extractedRoot = entries.find((name) => name.startsWith('ffmpeg-') && name.endsWith('-full_build'));
  if (!extractedRoot) {
    throw new Error(`Could not locate the extracted ffmpeg folder inside ${TMP_DIR} (found: ${entries.join(', ')})`);
  }

  const exePath = path.join(TMP_DIR, extractedRoot, 'bin', 'ffmpeg.exe');
  await mkdir(BIN_DIR, { recursive: true });
  await copyFile(exePath, path.join(BIN_DIR, 'ffmpeg.exe'));
  console.log(`Installed sofalizer-capable ffmpeg to ${path.join(BIN_DIR, 'ffmpeg.exe')} (gyan.dev full build, GPLv3).`);
}

async function setupMacIntel() {
  console.log('Fetching latest evermeet.cx ffmpeg snapshot info...');
  const infoRes = await fetch('https://evermeet.cx/ffmpeg/info/ffmpeg/release');
  if (!infoRes.ok) throw new Error('Failed to query evermeet.cx release info');
  const info = await infoRes.json();
  const downloadUrl = info?.download?.zip?.url;
  if (!downloadUrl) throw new Error('Unexpected evermeet.cx response shape — check https://evermeet.cx/ffmpeg/ manually');

  await mkdir(TMP_DIR, { recursive: true });
  const zipPath = path.join(TMP_DIR, 'ffmpeg.zip');
  await download(downloadUrl, zipPath);

  let unzipper;
  try {
    ({ default: unzipper } = await import('unzipper'));
  } catch {
    console.warn(`'unzipper' is not installed; extract ${zipPath} manually into ${BIN_DIR}/ffmpeg.`);
    return;
  }
  await mkdir(BIN_DIR, { recursive: true });
  const { createReadStream } = await import('node:fs');
  await createReadStream(zipPath).pipe(unzipper.Extract({ path: BIN_DIR })).promise();
  console.log(`Installed sofalizer-capable ffmpeg to ${BIN_DIR} (evermeet.cx build).`);
}

function printLinuxOrArmInstructions() {
  console.log(`
No off-the-shelf static ffmpeg build with libmysofa (required for the sofalizer
3D-audio filter) was confirmed for this platform (${process.platform}/${process.arch}).

Options:
  1. Build ffmpeg yourself with --enable-libmysofa, e.g. via:
       https://github.com/markus-perl/ffmpeg-build-script
  2. Run this bot via the provided Dockerfile, which builds an ffmpeg with
     libmysofa support from source for Linux.

Until then, the bot automatically falls back to the lightweight (non-HRTF)
spatial-audio filter chain — playback still works, just without true
binaural rendering. See README.md and the plan's ffmpeg support matrix.
`);
}

async function main() {
  try {
    if (process.platform === 'win32') {
      await setupWindows();
    } else if (process.platform === 'darwin' && process.arch === 'x64') {
      await setupMacIntel();
    } else {
      printLinuxOrArmInstructions();
    }
  } finally {
    await rm(TMP_DIR, { recursive: true, force: true }).catch(() => {});
  }
}

await main();
