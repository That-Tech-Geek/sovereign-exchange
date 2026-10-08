import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const constants = readFileSync("exchange_core/src/constants.rs", "utf8");
const contract = readFileSync("docs/PRODUCTION_CONTRACT.md", "utf8");
const gateway = readFileSync("api/_lib/engine.ts", "utf8");

if (!constants.includes("MAX_SOVEREIGNS: usize = 196")) {
  throw new Error("MAX_SOVEREIGNS must remain 196");
}
if (!constants.includes("MAX_INSTRUMENTS: usize = MAX_SOVEREIGNS * 2")) {
  throw new Error("MAX_INSTRUMENTS must remain sovereigns × spot/future");
}
if (!constants.includes("MAX_ORDERS: usize = 5_000_000")) {
  throw new Error("native MAX_ORDERS must remain 5,000,000");
}
if (!contract.includes("One matching owner")) {
  throw new Error("production contract lost single-writer ownership rule");
}
if (!contract.includes("Rejected commands must not consume canonical sequence numbers")) {
  throw new Error("production contract lost sequence rejection rule");
}
if (!gateway.includes("EXCHANGE_ENGINE_URL")) {
  throw new Error("Vercel gateway must route to the authoritative engine");
}
if (/MatchingEngine|OrderBook|OrderPool|accept_command|process_order/.test(gateway)) {
  throw new Error("Vercel gateway must not contain matching logic");
}

function walk(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });
}

for (const path of walk("api")) {
  if (!path.endsWith(".ts")) continue;
  const source = readFileSync(path, "utf8");
  if (/Math\.random|Date\.now\(|performance\.now\(|setInterval|setTimeout/.test(source)) {
    throw new Error(`non-deterministic/scheduled matching primitive found in API: ${path}`);
  }
}

console.log("Vercel architecture contract: PASS");
