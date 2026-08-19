import assert from "node:assert/strict";
import test from "node:test";

import { verifyReleaseVersions, versionFromReleaseTag } from "./verify-release-tag.mjs";

function metadata(version = "0.1.0") {
  return {
    workspace_members: ["core-id", "bot-id"],
    packages: [
      { id: "core-id", name: "pepeaudio-core", version },
      { id: "bot-id", name: "pepeaudio-bot", version },
      { id: "dependency-id", name: "serde", version: "1.0.0" },
    ],
  };
}

test("accepts stable and prerelease SemVer tags", () => {
  assert.equal(versionFromReleaseTag("v0.1.0"), "0.1.0");
  assert.equal(versionFromReleaseTag("v1.4.0-rc.2"), "1.4.0-rc.2");
});

test("rejects ambiguous release tags", () => {
  for (const tag of ["0.1.0", "v01.2.3", "v1.2", "v1.2.3+build", "v1.2.3-rc.02"]) {
    assert.throws(() => versionFromReleaseTag(tag));
  }
});

test("requires every first-party package to match the tag", () => {
  assert.equal(
    verifyReleaseVersions("v0.1.0", metadata(), { version: "0.1.0" }),
    "0.1.0",
  );
  assert.throws(
    () => verifyReleaseVersions("v0.2.0", metadata(), { version: "0.2.0" }),
    /Cargo packages/,
  );
});

test("requires the dashboard package to match the tag", () => {
  assert.throws(
    () => verifyReleaseVersions("v0.1.0", metadata(), { version: "0.2.0" }),
    /web\/package\.json/,
  );
});
