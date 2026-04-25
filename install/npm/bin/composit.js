#!/usr/bin/env node
// Launcher: resolve the platform-specific optional dependency and exec it.
"use strict";

const { execFileSync } = require("child_process");
const { join } = require("path");

const PLATFORMS = {
  "linux-x64":    "@nuetzliches/composit-linux-x64",
  "linux-arm64":  "@nuetzliches/composit-linux-arm64",
  "darwin-x64":   "@nuetzliches/composit-darwin-x64",
  "darwin-arm64": "@nuetzliches/composit-darwin-arm64",
  "win32-x64":    "@nuetzliches/composit-win32-x64",
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORMS[key];

if (!pkg) {
  process.stderr.write(`composit: unsupported platform ${key}\n`);
  process.exit(1);
}

let binDir;
try {
  binDir = require.resolve(`${pkg}/package.json`).replace(/package\.json$/, "");
} catch {
  process.stderr.write(
    `composit: optional dependency ${pkg} is not installed.\n` +
    `  Try: npm install -g @nuetzliches/composit\n`
  );
  process.exit(1);
}

const bin = join(binDir, "bin", process.platform === "win32" ? "composit.exe" : "composit");

try {
  execFileSync(bin, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  process.exit(e.status ?? 1);
}
