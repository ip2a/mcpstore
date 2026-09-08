#!/usr/bin/env node
"use strict";

const { spawnSync } = require("node:child_process");

const PLATFORM_PACKAGES = {
  "darwin-arm64": "@ip2a/mcpstore-bin-darwin-arm64",
  "linux-x64": "@ip2a/mcpstore-bin-linux-x64-gnu",
  "linux-arm64": "@ip2a/mcpstore-bin-linux-arm64-gnu",
  "win32-x64": "@ip2a/mcpstore-bin-win32-x64-msvc",
};

const key = `${process.platform}-${process.arch}`;
const packageName = PLATFORM_PACKAGES[key];
if (!packageName) {
  console.error(`[error] mcpstore does not support ${process.platform}-${process.arch}`);
  process.exit(1);
}

const binary = process.platform === "win32" ? "mcpstore.exe" : "mcpstore";
let binaryPath;
try {
  binaryPath = require.resolve(`${packageName}/bin/${binary}`);
} catch {
  console.error(`[error] Platform package ${packageName} is missing.`);
  console.error("[hint] Reinstall with optional dependencies enabled: npm install -g @ip2a/mcpstore");
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`[error] Failed to launch mcpstore: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
