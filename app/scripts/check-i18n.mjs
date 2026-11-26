import fs from "node:fs";
import { en } from "../src/i18n.js";

const source = fs.readFileSync(new URL("../src/App.svelte", import.meta.url), "utf8");
const re = /\bt\("((?:[^"\\]|\\.)*)"/g;
const missing = new Set();
const keys = new Set();
let match;
let calls = 0;

while ((match = re.exec(source))) {
  calls++;
  let key;
  try {
    key = JSON.parse(`"${match[1]}"`);
  } catch {
    key = match[1];
  }
  keys.add(key);
  if (!(key in en)) missing.add(key);
}

if (missing.size > 0) {
  console.error(`Missing ${missing.size} English translation(s):`);
  for (const key of [...missing].sort()) console.error(`  ${JSON.stringify(key)}`);
  process.exit(1);
}

console.log(`i18n OK: ${calls} calls, ${keys.size} unique strings`);
