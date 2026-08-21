import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  runtimeLicenseInventory,
  validateRuntimeLicenseInventory
} from "../web/build-license-assets.ts";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "..");
const webRoot = join(repositoryRoot, "web");

function fail(message) {
  throw new Error(`License verification failed: ${message}`);
}

function requireCondition(condition, message) {
  if (!condition) {
    fail(message);
  }
}

function readRepositoryFile(path) {
  return readFileSync(join(repositoryRoot, path), "utf8").replaceAll("\r\n", "\n");
}

function run(command, args, cwd = repositoryRoot) {
  let executable = command;
  let processArguments = args;
  if (process.platform === "win32") {
    const words = [command, ...args];
    requireCondition(
      words.every((word) => /^[A-Za-z0-9._:@/\\-]+$/.test(word)),
      "an unsafe Windows process argument reached the license verifier"
    );
    executable = process.env.ComSpec ?? "cmd.exe";
    processArguments = ["/d", "/s", "/c", words.join(" ")];
  }
  const result = spawnSync(executable, processArguments, {
    cwd,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["ignore", "pipe", "inherit"]
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    fail(`${command} exited with code ${result.status}`);
  }
  return result.stdout;
}

const expectedMitLicense = `MIT License

Copyright (c) 2026 SumireLabs, s12kuma01

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
`;

requireCondition(
  readRepositoryFile("LICENSE") === expectedMitLicense,
  "the root LICENSE is not the approved MIT text and copyright notice"
);

const metadata = JSON.parse(
  run("cargo", ["metadata", "--locked", "--format-version", "1"])
);
const workspaceIds = new Set(metadata.workspace_members);
const workspacePackages = metadata.packages.filter(({ id }) => workspaceIds.has(id));

requireCondition(workspacePackages.length > 0, "the Cargo workspace has no packages");
const invalidWorkspacePackages = workspacePackages.filter(({ license }) => license !== "MIT");
requireCondition(
  invalidWorkspacePackages.length === 0,
  `first-party Cargo packages without MIT metadata: ${invalidWorkspacePackages
    .map(({ name }) => name)
    .join(", ")}`
);

const thirdPartyPackages = metadata.packages.filter(({ id }) => !workspaceIds.has(id));
const missingCargoMetadata = thirdPartyPackages.filter(
  ({ license, license_file: licenseFile }) => !license && !licenseFile
);
requireCondition(
  missingCargoMetadata.length === 0,
  `third-party Cargo packages without license metadata: ${missingCargoMetadata
    .map(({ name, version }) => `${name}@${version}`)
    .join(", ")}`
);

const vendorDirectory = join(repositoryRoot, "vendor", "hpke-rs-0.6.1-security-patch");
const hpkePackage = thirdPartyPackages.find(
  ({ name, manifest_path: manifestPath }) =>
    name === "hpke-rs" && resolve(manifestPath).startsWith(vendorDirectory)
);
requireCondition(hpkePackage?.license === "MPL-2.0", "vendored hpke-rs must remain MPL-2.0");
for (const path of ["Cargo.toml", "LICENSE-MPL-2.0", "PATCH.md", join("src", "lib.rs")]) {
  requireCondition(existsSync(join(vendorDirectory, path)), `vendored hpke-rs is missing ${path}`);
}

const webPackage = JSON.parse(readRepositoryFile(join("web", "package.json")));
requireCondition(webPackage.private === true, "the dashboard package must remain private");
requireCondition(webPackage.license === "MIT", "the dashboard package must declare MIT");

const pnpmLicenseGroups = JSON.parse(
  run("pnpm", ["licenses", "list", "--prod", "--json"], webRoot)
);
const actualWebPackages = new Set();
for (const [license, packages] of Object.entries(pnpmLicenseGroups)) {
  for (const packageEntry of packages) {
    for (const version of packageEntry.versions) {
      actualWebPackages.add(`${packageEntry.name}@${version}|${license}`);
    }
  }
}
const approvedWebPackages = new Set(
  runtimeLicenseInventory.map(({ name, version, license }) => `${name}@${version}|${license}`)
);
requireCondition(
  approvedWebPackages.size === runtimeLicenseInventory.length,
  "the approved Web runtime license inventory contains duplicate packages"
);
const unapprovedWebPackages = [...actualWebPackages].filter(
  (entry) => !approvedWebPackages.has(entry)
);
const missingWebPackages = [...approvedWebPackages].filter(
  (entry) => !actualWebPackages.has(entry)
);
requireCondition(
  unapprovedWebPackages.length === 0 && missingWebPackages.length === 0,
  `Web runtime license inventory changed; unapproved=[${unapprovedWebPackages.join(", ")}], ` +
    `missing=[${missingWebPackages.join(", ")}]`
);
validateRuntimeLicenseInventory(webRoot);

const rustDockerfile = readRepositoryFile(join("deploy", "rust", "Dockerfile"));
for (const requiredText of [
  "COPY LICENSE /usr/share/licenses/pepeaudio/LICENSE",
  "COPY docs/third-party.md /usr/share/doc/pepeaudio/THIRD-PARTY.md",
  "COPY vendor/hpke-rs-0.6.1-security-patch /usr/share/src/pepeaudio/hpke-rs-0.6.1-security-patch"
]) {
  requireCondition(rustDockerfile.includes(requiredText), `Rust image is missing: ${requiredText}`);
}

const botDockerfile = readRepositoryFile(join("deploy", "rust", "Dockerfile.bot"));
for (const requiredText of [
  "ARG YTDLP_VERSION=2026.08.19",
  "ARG YTDLP_SHA256=1fa6733c37ea6fb51c99ad8fe785e7b7e5f3246c9b980230329d4fb72ed8d4d6",
  "ARG YTDLP_LICENSE_SHA256=7e12e5df4bae12cb21581ba157ced20e1986a0508dd10d0e8a4ab9a4cf94e85c",
  "ARG YTDLP_NOTICES_SHA256=472aefe951c7db35e1657c1d13fd337140511ed6f2b329205105ad441c5a02b7",
  "ARG DENO_VERSION=2.8.1",
  "ARG DENO_AMD64_SHA256=2d7bb6195226ac832e0bf7109a115f0af65ee69ac797a4bbde5b27a06cc242d9",
  "ARG DENO_ARM64_SHA256=67e9df91870fd0af700df924173e3009ea7ff6956e2c3c3bb86065d6070d0fd6",
  "COPY --from=media-tools /out/licenses/yt-dlp /usr/share/licenses/yt-dlp",
  "COPY --from=media-tools /out/licenses/deno /usr/share/licenses/deno",
  "COPY docs/licenses/deno-2.8.1-NOTICE.md /usr/share/licenses/deno/PEPEAUDIO-NOTICE.md",
  "apt-get install --yes --no-install-recommends ca-certificates ffmpeg libssl3 python3",
]) {
  requireCondition(botDockerfile.includes(requiredText), `Bot image is missing: ${requiredText}`);
}
requireCondition(
  !botDockerfile.includes("yt-dlp_linux"),
  "Bot image must use the generic yt-dlp zipimport artifact, not a PyInstaller bundle"
);
requireCondition(
  readRepositoryFile(join("docs", "licenses", "deno-2.8.1-NOTICE.md")).includes(
    "does not publish a consolidated"
  ),
  "Deno's upstream third-party-notice boundary is undocumented"
);

const caddyDockerfile = readRepositoryFile(join("deploy", "caddy", "Dockerfile"));
for (const requiredText of [
  "COPY --from=web-builder /web/dist /srv",
  "COPY LICENSE /usr/share/licenses/pepeaudio/LICENSE",
  "COPY docs/third-party.md /usr/share/doc/pepeaudio/THIRD-PARTY.md",
  "COPY --from=web-builder /web/dist/licenses /usr/share/licenses/pepeaudio-web-dependencies"
]) {
  requireCondition(caddyDockerfile.includes(requiredText), `Caddy image is missing: ${requiredText}`);
}

const webLicenseBuild = readRepositoryFile(join("web", "build-license-assets.ts"));
for (const requiredText of [
  "LICENSE.txt",
  "THIRD-PARTY.md",
  "manifest.json",
  "validateRuntimeLicenseInventory"
]) {
  requireCondition(
    webLicenseBuild.includes(requiredText),
    `the standalone Web build does not account for ${requiredText}`
  );
}

console.log(
  `License verification passed: ${workspacePackages.length} first-party Cargo packages, ` +
    `${thirdPartyPackages.length} third-party Cargo packages, and ` +
    `${actualWebPackages.size} Web runtime packages.`
);
console.log(`Repository: ${relative(process.cwd(), repositoryRoot) || "."}`);
