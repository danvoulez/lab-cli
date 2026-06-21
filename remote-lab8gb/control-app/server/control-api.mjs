// control-api — the sidecar that gives control.minilab.work a real backend.
//
// Modeled on passport-api.mjs (the proven pattern on the same box): one
// dependency-free node process serves the built SPA (static dist/) AND a small
// /api/* surface. It is the only process here holding spine credentials; reads
// go to the real ledger over PostgREST with the server-side secret key, which
// never leaves this box.
//
// Honesty law (this whole effort): no surface says "OK" without evidence.
//   /api/health  is PUBLIC and aggregate-only (status + act count + build).
//   /api/receipts and /api/ghosts read ledger CONTENT and are AUTH-GATED,
//   mirroring passport's posture (a Supabase session token is required).
//
// Env (required):
//   SANTOANDRE_SUPABASE_URL         spine PostgREST base
//   SANTOANDRE_SUPABASE_SECRET_KEY  spine secret (read; never leaves this box)
// Env (required for the auth-gated reads):
//   AUTH_SUPABASE_URL               auth organ base (minilab.database)
//   AUTH_SUPABASE_KEY               auth organ publishable key
// Optional:
//   CONTROL_DIST_DIR                built SPA to serve (default ./dist)
//   PORT                            listen port (default 4173 — cloudflared target)
//   ACTGRAPH_ENV                    environment label for the strip (default "production")

import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const SPINE_URL = need("SANTOANDRE_SUPABASE_URL");
const SPINE_KEY = need("SANTOANDRE_SUPABASE_SECRET_KEY");
const AUTH_URL = process.env.AUTH_SUPABASE_URL ?? "";
const AUTH_KEY = process.env.AUTH_SUPABASE_KEY ?? "";
const DIST_DIR = process.env.CONTROL_DIST_DIR ?? path.join(process.cwd(), "dist");
const PORT = Number(process.env.PORT ?? 4173);
const ENV = process.env.ACTGRAPH_ENV ?? "production";

// Build provenance — written at deploy time on the draft node (lab-256, where
// git exists) and shipped beside this file. Absent => honest "unknown".
const BUILD = readBuildInfo();

const MIME = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8", ".json": "application/json",
  ".svg": "image/svg+xml", ".png": "image/png", ".ico": "image/x-icon",
  ".woff2": "font/woff2", ".woff": "font/woff", ".map": "application/json",
};

function need(name) {
  const v = process.env[name];
  if (!v) { console.error(`missing env: ${name}`); process.exit(2); }
  return v;
}

function readBuildInfo() {
  try {
    const raw = fs.readFileSync(path.join(path.dirname(new URL(import.meta.url).pathname), "build-info.json"), "utf8");
    const b = JSON.parse(raw);
    return { commit: b.commit ?? "unknown", commit_time: b.commit_time ?? "", built_at: b.built_at ?? "" };
  } catch {
    return { commit: "unknown", commit_time: "", built_at: "" };
  }
}

// Read the real ledger. logline_acts exposes the nine slots as columns
// (who/did/this/when/status) plus content_hash and aux. Read-only; a window
// onto the lab's memory, never authority.
async function spine(query, extraHeaders = {}) {
  const r = await fetch(`${SPINE_URL}/rest/v1/logline_acts?${query}`, {
    headers: { apikey: SPINE_KEY, authorization: `Bearer ${SPINE_KEY}`, ...extraHeaders },
  });
  if (!r.ok) throw new Error(`spine ${r.status}`);
  return r;
}

async function authUser(token) {
  if (!token || !AUTH_URL || !AUTH_KEY) return null;
  const r = await fetch(`${AUTH_URL}/auth/v1/user`, {
    headers: { apikey: AUTH_KEY, authorization: `Bearer ${token}` },
  });
  if (!r.ok) return null;
  const u = await r.json();
  return u && u.id ? { subject: u.id, email: u.email ?? "" } : null;
}

// PUBLIC, aggregate-only: enough to make the strip honest, no ledger content.
async function health() {
  let spine_ok = false;
  let acts_total = null;
  try {
    const r = await spine("select=content_hash&limit=1", { Range: "0-0", Prefer: "count=exact" });
    spine_ok = true;
    const cr = r.headers.get("content-range") ?? ""; // e.g. "0-0/326"
    const m = cr.match(/\/(\d+)\s*$/);
    if (m) acts_total = Number(m[1]);
    await r.text();
  } catch {
    spine_ok = false;
  }
  return {
    surface: "control.minilab.work",
    status: spine_ok ? "live" : "degraded",
    spine_ok,
    data_source: "santo-andré spine · logline_acts (PostgREST, read-only)",
    acts_total,
    build: BUILD.commit,
    commit_time: BUILD.commit_time,
    built_at: BUILD.built_at,
    environment: ENV,
    generated_at: new Date().toISOString(),
    reads_gated: true,
  };
}

function flatten(rows) {
  return rows.map((r) => ({
    hash: r.content_hash,
    who: r.who ?? "",
    did: r.did ?? "",
    this: r.this ?? "",
    when: r.when ?? "",
    status: r.status ?? "",
  }));
}

// AUTH-GATED: real recent admitted Acts (the lab's memory, newest slice).
async function receipts(limit = 30) {
  const r = await spine(
    `select=content_hash,who,did,this,when,status&order=inserted_at.desc&limit=${limit}`
  );
  const rows = await r.json();
  return { receipts: flatten(rows), data_source: "logline_acts", fetched_at: new Date().toISOString() };
}

// AUTH-GATED: ghosts — acts opened when evidence/authority was missing. The
// ledger may honestly hold few or none; the cockpit shows the true count.
async function ghosts(limit = 30) {
  const r = await spine(
    `select=content_hash,who,did,this,when,status&or=(did.ilike.*ghost*,status.eq.ghost)&order=inserted_at.desc&limit=${limit}`
  );
  const rows = await r.json();
  return { ghosts: flatten(rows), data_source: "logline_acts", fetched_at: new Date().toISOString() };
}

function serveStatic(req, res) {
  let rel = decodeURIComponent((req.url ?? "/").split("?")[0]);
  if (rel === "/" || rel === "") rel = "/index.html";
  const full = path.join(DIST_DIR, path.normalize(rel));
  if (!full.startsWith(DIST_DIR)) { res.writeHead(403); res.end(); return; }
  fs.readFile(full, (err, buf) => {
    if (err) {
      fs.readFile(path.join(DIST_DIR, "index.html"), (e2, idx) => {
        if (e2) { res.writeHead(404); res.end("not found"); return; }
        res.writeHead(200, { "content-type": MIME[".html"] });
        res.end(idx);
      });
      return;
    }
    res.writeHead(200, { "content-type": MIME[path.extname(full)] ?? "application/octet-stream" });
    res.end(buf);
  });
}

const server = http.createServer(async (req, res) => {
  const send = (code, body) => {
    res.writeHead(code, { "content-type": "application/json" });
    res.end(JSON.stringify(body));
  };
  try {
    if (!req.url?.startsWith("/api/")) return serveStatic(req, res);

    // PUBLIC, aggregate-only.
    if (req.method === "GET" && req.url.startsWith("/api/health")) {
      return send(200, await health());
    }

    // Everything else reading ledger content is auth-gated (passport posture).
    const auth = req.headers.authorization ?? "";
    const token = auth.startsWith("Bearer ") ? auth.slice(7) : "";
    const user = await authUser(token);
    if (!user) return send(401, { error: "ledger reads require a signed-in session", reads_gated: true });

    if (req.method === "GET" && req.url.startsWith("/api/receipts")) {
      return send(200, await receipts(30));
    }
    if (req.method === "GET" && req.url.startsWith("/api/ghosts")) {
      return send(200, await ghosts(30));
    }
    return send(404, { error: "not found" });
  } catch (e) {
    return send(502, { error: String(e?.message ?? e) });
  }
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`control on 127.0.0.1:${PORT} — SPA(${DIST_DIR}) + /api · build ${BUILD.commit} · env ${ENV}`);
});
