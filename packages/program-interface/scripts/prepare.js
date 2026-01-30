#!/usr/bin/env node
/**
 * Copies IDL and types from anchor build output to src/
 * Run this before building the package
 */

import { copyFileSync, existsSync, mkdirSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, "..");
const repoRoot = resolve(rootDir, "../..");

const files = [
  {
    src: resolve(repoRoot, "target/idl/omnipair.json"),
    dest: resolve(rootDir, "src/idl.json"),
  },
  {
    src: resolve(repoRoot, "target/types/omnipair.ts"),
    dest: resolve(rootDir, "src/types.ts"),
  },
];

console.log("Preparing @omnipair/program-interface...\n");

for (const { src, dest } of files) {
  if (!existsSync(src)) {
    console.error(`ERROR: Source file not found: ${src}`);
    console.error("Run 'anchor build' first to generate IDL and types.");
    process.exit(1);
  }

  // Ensure destination directory exists
  mkdirSync(dirname(dest), { recursive: true });

  copyFileSync(src, dest);
  console.log(`✓ Copied ${src.split("/").pop()} -> src/`);
}

console.log("\nDone! Ready to build.");
