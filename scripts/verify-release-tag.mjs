import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const RELEASE_TAG = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

export function versionFromReleaseTag(tag) {
  const match = RELEASE_TAG.exec(tag);
  if (match === null) {
    throw new Error(
      `release tag must use vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-prerelease: ${tag}`,
    );
  }

  const prerelease = match[4];
  if (
    prerelease !== undefined &&
    prerelease.split(".").some((part) => /^\d+$/.test(part) && part.length > 1 && part[0] === "0")
  ) {
    throw new Error(`numeric prerelease identifiers must not have leading zeroes: ${tag}`);
  }

  return tag.slice(1);
}

export function verifyReleaseVersions(tag, cargoMetadata, webPackage) {
  const expected = versionFromReleaseTag(tag);
  const workspaceMembers = new Set(cargoMetadata.workspace_members);
  const workspacePackages = cargoMetadata.packages.filter(({ id }) => workspaceMembers.has(id));
  if (workspacePackages.length === 0) {
    throw new Error("Cargo metadata contains no workspace packages");
  }

  const mismatched = workspacePackages
    .filter(({ version }) => version !== expected)
    .map(({ name, version }) => `${name}@${version}`)
    .sort();
  if (mismatched.length > 0) {
    throw new Error(
      `release tag ${tag} does not match Cargo packages: ${mismatched.join(", ")}`,
    );
  }
  if (webPackage.version !== expected) {
    throw new Error(
      `release tag ${tag} does not match web/package.json version ${webPackage.version}`,
    );
  }

  return expected;
}

function readCargoMetadata() {
  const output = execFileSync(
    "cargo",
    ["metadata", "--format-version", "1", "--no-deps", "--locked"],
    { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
  );
  return JSON.parse(output);
}

function main() {
  const webPackage = JSON.parse(readFileSync("web/package.json", "utf8"));
  const tag = process.argv[2] ?? `v${webPackage.version}`;
  const version = verifyReleaseVersions(tag, readCargoMetadata(), webPackage);
  process.stdout.write(`Release version verified: ${tag} (${version})\n`);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`Release version check failed: ${message}\n`);
    process.exitCode = 1;
  }
}
