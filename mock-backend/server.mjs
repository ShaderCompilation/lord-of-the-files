// Minimal mock of the AI rename backend, implementing the contract in
// src/lib/types.ts (POST /v1/rename). Run with: pnpm mock-backend
//
// This is a stand-in for the real (future) service. It does NOT call an LLM — it just
// produces human-readable Title Case names so the AI step can be exercised offline.

import { createServer } from "node:http";

const PORT = process.env.PORT ? Number(process.env.PORT) : 8787;

/** Turn "IMG_0001-final" into "Img 0001 Final". */
function humanize(name) {
  return name
    .replace(/[_\-.]+/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .trim()
    .replace(/\s+/g, " ")
    .split(" ")
    .map((w) => (w ? w[0].toUpperCase() + w.slice(1).toLowerCase() : w))
    .join(" ");
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let data = "";
    req.on("data", (c) => (data += c));
    req.on("end", () => resolve(data));
    req.on("error", reject);
  });
}

const server = createServer(async (req, res) => {
  res.setHeader("Content-Type", "application/json");
  if (req.method !== "POST" || !req.url?.startsWith("/v1/rename")) {
    res.statusCode = 404;
    res.end(JSON.stringify({ version: 1, results: [], error: { code: "not_found", message: "Use POST /v1/rename" } }));
    return;
  }

  try {
    const body = JSON.parse((await readBody(req)) || "{}");
    const maxLen = body.options?.maxLen ?? 80;
    const results = (body.files ?? []).map((f) => ({
      id: f.id,
      newName: humanize(f.name).slice(0, maxLen) || f.name,
    }));
    console.log(`[mock] prompt=${JSON.stringify(body.prompt)} files=${results.length}`);
    res.statusCode = 200;
    res.end(JSON.stringify({ version: 1, results, error: null }));
  } catch (e) {
    res.statusCode = 400;
    res.end(JSON.stringify({ version: 1, results: [], error: { code: "bad_request", message: String(e) } }));
  }
});

server.listen(PORT, () => {
  console.log(`Mock AI backend listening on http://localhost:${PORT}/v1/rename`);
});
