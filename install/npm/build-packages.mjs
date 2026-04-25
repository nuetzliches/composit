#!/usr/bin/env node
// Generate npm publish-ready directories for the meta package
// (@nutz/composit) and the five platform sub-packages from a directory
// of release artifacts (.tar.gz / .zip).
//
// usage: build-packages.mjs <version> <artifacts-dir> <out-dir>
"use strict";

import { execSync } from "node:child_process";
import {
  chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync,
  rmSync, writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [, , versionArg, artifactsArg, outArg] = process.argv;
if (!versionArg || !artifactsArg || !outArg) {
  console.error("usage: build-packages.mjs <version> <artifacts-dir> <out-dir>");
  process.exit(2);
}

const here = dirname(fileURLToPath(import.meta.url));
const version = versionArg;
const artifactsDir = resolve(artifactsArg);
const outDir = resolve(outArg);

const PLATFORMS = [
  { key: "linux-x64",    archive: "composit-x86_64-unknown-linux-musl.tar.gz",  os: "linux",  cpu: "x64",   binary: "composit" },
  { key: "linux-arm64",  archive: "composit-aarch64-unknown-linux-musl.tar.gz", os: "linux",  cpu: "arm64", binary: "composit" },
  { key: "darwin-x64",   archive: "composit-x86_64-apple-darwin.tar.gz",        os: "darwin", cpu: "x64",   binary: "composit" },
  { key: "darwin-arm64", archive: "composit-aarch64-apple-darwin.tar.gz",       os: "darwin", cpu: "arm64", binary: "composit" },
  { key: "win32-x64",    archive: "composit-x86_64-pc-windows-msvc.zip",        os: "win32",  cpu: "x64",   binary: "composit.exe" },
];

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

for (const p of PLATFORMS) {
  const src = join(artifactsDir, p.archive);
  if (!existsSync(src)) {
    console.error(`missing artifact: ${src}`);
    process.exit(1);
  }

  const pkgDir = join(outDir, p.key);
  const binDir = join(pkgDir, "bin");
  mkdirSync(binDir, { recursive: true });

  const stage = join(outDir, `_stage-${p.key}`);
  mkdirSync(stage, { recursive: true });
  if (p.archive.endsWith(".zip")) {
    execSync(`unzip -o "${src}" -d "${stage}"`, { stdio: "inherit" });
  } else {
    execSync(`tar -xzf "${src}" -C "${stage}"`, { stdio: "inherit" });
  }
  copyFileSync(join(stage, p.binary), join(binDir, p.binary));
  chmodSync(join(binDir, p.binary), 0o755);
  rmSync(stage, { recursive: true, force: true });

  const subPkg = {
    name: `@nutz/composit-${p.key}`,
    version,
    description: `composit binary for ${p.os}-${p.cpu}`,
    homepage: "https://nuetzliches.github.io/composit",
    repository: { type: "git", url: "https://github.com/nuetzliches/composit.git" },
    license: "MIT",
    os: [p.os],
    cpu: [p.cpu],
    files: ["bin/"],
  };
  writeFileSync(join(pkgDir, "package.json"), JSON.stringify(subPkg, null, 2) + "\n");
}

const meta = JSON.parse(readFileSync(join(here, "package.json"), "utf8"));
meta.version = version;
for (const p of PLATFORMS) {
  meta.optionalDependencies[`@nutz/composit-${p.key}`] = version;
}

const metaDir = join(outDir, "meta");
mkdirSync(join(metaDir, "bin"), { recursive: true });
writeFileSync(join(metaDir, "package.json"), JSON.stringify(meta, null, 2) + "\n");
copyFileSync(join(here, "bin", "composit.js"), join(metaDir, "bin", "composit.js"));
chmodSync(join(metaDir, "bin", "composit.js"), 0o755);

console.log(`built ${PLATFORMS.length} sub-packages + meta in ${outDir}`);
