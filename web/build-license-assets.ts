import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  writeFileSync
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { Plugin } from "vite";

type LicenseSource = Readonly<{
  name: string;
  version: string;
  file: string;
  sha256: string;
  upstreamUrl?: string;
}>;

export type RuntimeLicense = Readonly<{
  name: string;
  version: string;
  license: string;
  source: LicenseSource;
}>;

function packaged(
  name: string,
  version: string,
  license: string,
  file: string,
  sha256: string
): RuntimeLicense {
  return { name, version, license, source: { name, version, file, sha256 } };
}

function upstreamFallback(
  name: string,
  version: string,
  license: string,
  source: LicenseSource
): RuntimeLicense {
  return { name, version, license, source };
}

export const runtimeLicenseInventory = [
  upstreamFallback("@astryxdesign/core", "0.3.0", "MIT", {
    name: "@astryxdesign/theme-neutral",
    version: "0.3.0",
    file: "LICENSE",
    sha256: "a6855be541fc8f446acd1bc4f2f8efce1ace6dce71dba32fcd8da553ee54b473",
    upstreamUrl: "https://github.com/facebook/astryx/blob/v0.3.0/LICENSE"
  }),
  packaged(
    "@astryxdesign/theme-neutral",
    "0.3.0",
    "MIT",
    "LICENSE",
    "a6855be541fc8f446acd1bc4f2f8efce1ace6dce71dba32fcd8da553ee54b473"
  ),
  packaged(
    "@dnd-kit/accessibility",
    "3.1.1",
    "MIT",
    "LICENSE",
    "537607e3f1533ad2c5b12978967f5ab27f34ffb7e626e321017a7ccca6cc24ff"
  ),
  packaged(
    "@dnd-kit/core",
    "6.3.1",
    "MIT",
    "LICENSE",
    "537607e3f1533ad2c5b12978967f5ab27f34ffb7e626e321017a7ccca6cc24ff"
  ),
  packaged(
    "@dnd-kit/sortable",
    "10.0.0",
    "MIT",
    "LICENSE",
    "537607e3f1533ad2c5b12978967f5ab27f34ffb7e626e321017a7ccca6cc24ff"
  ),
  packaged(
    "@dnd-kit/utilities",
    "3.2.2",
    "MIT",
    "LICENSE",
    "537607e3f1533ad2c5b12978967f5ab27f34ffb7e626e321017a7ccca6cc24ff"
  ),
  packaged(
    "@formatjs/fast-memoize",
    "3.1.7",
    "MIT",
    "LICENSE.md",
    "b1cc980e28e9e48f377db191578a9361494ca7117185b69dd66cabaf5b460fb7"
  ),
  packaged(
    "@formatjs/icu-messageformat-parser",
    "3.5.16",
    "MIT",
    "LICENSE.md",
    "b1cc980e28e9e48f377db191578a9361494ca7117185b69dd66cabaf5b460fb7"
  ),
  packaged(
    "@formatjs/icu-skeleton-parser",
    "2.1.11",
    "MIT",
    "LICENSE.md",
    "b1cc980e28e9e48f377db191578a9361494ca7117185b69dd66cabaf5b460fb7"
  ),
  upstreamFallback("@stylexjs/stylex", "0.19.0", "MIT", {
    name: "react",
    version: "19.2.8",
    file: "LICENSE",
    sha256: "da6d3703ed11cbe42bd212c725957c98da23cbff1998c05fa4b3d976d1a58e93",
    upstreamUrl: "https://github.com/facebook/stylex/blob/0.19.0/LICENSE"
  }),
  packaged(
    "css-mediaquery",
    "0.1.2",
    "BSD",
    "LICENSE",
    "d3dfa68a3c80e64eb10e46cef10e4208502da5ffb1387b11db00e42a507ab8f7"
  ),
  packaged(
    "intl-messageformat",
    "11.2.13",
    "BSD-3-Clause",
    "LICENSE.md",
    "cc4143615b27c66eafbe913f132a9e41ac14c1e2ec6b2df24dafeaa97f028850"
  ),
  packaged(
    "invariant",
    "2.2.4",
    "MIT",
    "LICENSE",
    "f657f99d3fb9647db92628e96007aabb46e5f04f33e49999075aab8e250ca7ce"
  ),
  packaged(
    "js-tokens",
    "4.0.0",
    "MIT",
    "LICENSE",
    "2213d91c606205c71eb051a199478cdc2adde945893404d7f1421436dd6d5cc1"
  ),
  packaged(
    "loose-envify",
    "1.4.0",
    "MIT",
    "LICENSE",
    "4eb7543b08d955a6d23fcc224601d43ff566e775be918805e26210d7f6eb4893"
  ),
  packaged(
    "lucide-react",
    "1.31.0",
    "ISC",
    "LICENSE",
    "b495047bd93a9b06913511076f504daba17d5bbeb3e0650f3bb53a4220329c57"
  ),
  packaged(
    "react",
    "19.2.8",
    "MIT",
    "LICENSE",
    "da6d3703ed11cbe42bd212c725957c98da23cbff1998c05fa4b3d976d1a58e93"
  ),
  packaged(
    "react-dom",
    "19.2.8",
    "MIT",
    "LICENSE",
    "da6d3703ed11cbe42bd212c725957c98da23cbff1998c05fa4b3d976d1a58e93"
  ),
  packaged(
    "scheduler",
    "0.27.0",
    "MIT",
    "LICENSE",
    "da6d3703ed11cbe42bd212c725957c98da23cbff1998c05fa4b3d976d1a58e93"
  ),
  packaged(
    "styleq",
    "0.2.1",
    "MIT",
    "LICENSE",
    "b00770e3c5d6a029cb104f7ac0d328c88ec684d31e8188c61ca91fcc17743cdc"
  ),
  packaged(
    "tslib",
    "2.8.1",
    "0BSD",
    "LICENSE.txt",
    "210b19e543130388c68654b7497e967119ce17145f66ab7d85688fbd70f08751"
  )
] as const satisfies readonly RuntimeLicense[];

const recognizedLicenseFiles = [
  "LICENSE",
  "LICENSE.md",
  "LICENSE.txt",
  "LICENCE",
  "LICENCE.md",
  "LICENCE.txt",
  "COPYING",
  "COPYING.md",
  "NOTICE",
  "NOTICE.md"
];

function packageManifest(packageRoot: string): { name?: string; version?: string; license?: string } {
  return JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
}

function installedPackageRoot(nodeModules: string, name: string, version: string): string {
  const virtualStore = join(nodeModules, ".pnpm");
  const packagePath = name.split("/");
  const roots = new Set<string>();

  for (const entry of readdirSync(virtualStore, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const candidate = join(virtualStore, entry.name, "node_modules", ...packagePath);
    if (!existsSync(join(candidate, "package.json"))) continue;
    const manifest = packageManifest(candidate);
    if (manifest.name === name && manifest.version === version) roots.add(realpathSync(candidate));
  }

  if (roots.size !== 1) {
    throw new Error(`expected one installed ${name}@${version} root, found ${roots.size}`);
  }
  return [...roots][0];
}

function checkedLicenseSource(nodeModules: string, entry: RuntimeLicense): string {
  const targetRoot = installedPackageRoot(nodeModules, entry.name, entry.version);
  const targetManifest = packageManifest(targetRoot);
  if (targetManifest.license !== entry.license) {
    throw new Error(`${entry.name}@${entry.version} no longer declares ${entry.license}`);
  }

  const usesFallback = entry.name !== entry.source.name || entry.version !== entry.source.version;
  if (usesFallback) {
    if (entry.source.upstreamUrl === undefined) {
      throw new Error(`${entry.name}@${entry.version} fallback has no upstream license URL`);
    }
    const ownNotices = recognizedLicenseFiles.filter((file) => existsSync(join(targetRoot, file)));
    if (ownNotices.length > 0) {
      throw new Error(`${entry.name}@${entry.version} now ships ${ownNotices.join(", ")}; remove fallback`);
    }
  }

  const sourceRoot = installedPackageRoot(nodeModules, entry.source.name, entry.source.version);
  const sourceManifest = packageManifest(sourceRoot);
  if (sourceManifest.license !== entry.license) {
    throw new Error(`${entry.source.name}@${entry.source.version} is not a ${entry.license} source`);
  }

  const source = join(sourceRoot, entry.source.file);
  const digest = createHash("sha256").update(readFileSync(source)).digest("hex");
  if (digest !== entry.source.sha256) {
    throw new Error(`${entry.name}@${entry.version} license digest changed: ${digest}`);
  }
  return source;
}

export function validateRuntimeLicenseInventory(
  root: string
): readonly Readonly<{ entry: RuntimeLicense; source: string }>[] {
  const nodeModules = join(root, "node_modules");
  return runtimeLicenseInventory.map((entry) => ({
    entry,
    source: checkedLicenseSource(nodeModules, entry)
  }));
}

export function licenseAssets(): Plugin {
  return {
    name: "pepeaudio-license-assets",
    apply: "build",
    closeBundle() {
      const webRoot = process.cwd();
      const repositoryRoot = resolve(webRoot, "..");
      const output = join(webRoot, "dist");
      const licenseOutput = join(output, "licenses");
      mkdirSync(licenseOutput, { recursive: true });

      copyFileSync(join(repositoryRoot, "LICENSE"), join(output, "LICENSE.txt"));
      copyFileSync(join(repositoryRoot, "docs", "third-party.md"), join(output, "THIRD-PARTY.md"));

      const manifest = validateRuntimeLicenseInventory(webRoot).map(({ entry, source }) => {
        const destination = join(licenseOutput, ...entry.name.split("/"), "LICENSE");
        mkdirSync(dirname(destination), { recursive: true });
        copyFileSync(source, destination);
        return {
          name: entry.name,
          version: entry.version,
          license: entry.license,
          notice: `${entry.name}/LICENSE`,
          sourcePackage: `${entry.source.name}@${entry.source.version}`,
          sourceFile: entry.source.file,
          sha256: entry.source.sha256,
          ...(entry.source.upstreamUrl === undefined
            ? {}
            : { upstreamLicense: entry.source.upstreamUrl })
        };
      });

      writeFileSync(
        join(licenseOutput, "manifest.json"),
        `${JSON.stringify({ schemaVersion: 1, packages: manifest }, null, 2)}\n`,
        "utf8"
      );
    }
  };
}
