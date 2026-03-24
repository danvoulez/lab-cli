const http = require("http");
const fs = require("fs");
const fsp = fs.promises;
const path = require("path");

const { BACKUP_DIR, OFFICIALIZE_DIR, ensureDirs, processPaths } = require("./processor");

const PORT = Number(process.env.PORT || 4319);
const HOST = "127.0.0.1";

const PUBLIC_DIR = path.join(__dirname, "public");

function json(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
    "cache-control": "no-store",
  });
  res.end(body);
}

function parseBody(req) {
  return new Promise((resolve, reject) => {
    let raw = "";
    req.on("data", (chunk) => {
      raw += chunk;
      if (raw.length > 1024 * 1024) {
        reject(new Error("Body muito grande"));
      }
    });
    req.on("end", () => {
      try {
        resolve(raw ? JSON.parse(raw) : {});
      } catch (err) {
        reject(err);
      }
    });
    req.on("error", reject);
  });
}

async function handleProcess(req, res) {
  try {
    const body = await parseBody(req);
    const mode = body.mode;
    const paths = body.paths;
    const result = await processPaths(mode, paths);
    return json(res, 200, result);
  } catch (err) {
    return json(res, 400, { error: err.message || "erro" });
  }
}

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
};

async function serveStatic(req, res) {
  let reqPath = req.url.split("?")[0];
  if (reqPath === "/") reqPath = "/index.html";
  const safePath = path.normalize(reqPath).replace(/^(\.\.[/\\])+/, "");
  const abs = path.join(PUBLIC_DIR, safePath);

  try {
    const stat = await fsp.stat(abs);
    if (!stat.isFile()) throw new Error("not file");
    const ext = path.extname(abs).toLowerCase();
    const mime = MIME[ext] || "application/octet-stream";
    const content = await fsp.readFile(abs);
    res.writeHead(200, {
      "content-type": mime,
      "cache-control": "no-store",
      "content-length": content.length,
    });
    res.end(content);
  } catch {
    if (reqPath !== "/index.html") {
      req.url = "/index.html";
      return serveStatic(req, res);
    }
    res.writeHead(404);
    res.end("Not found");
  }
}

async function bootstrap() {
  await ensureDirs();
  const server = http.createServer(async (req, res) => {
    if (req.method === "POST" && req.url === "/api/process") {
      return handleProcess(req, res);
    }
    if (req.method === "GET") {
      return serveStatic(req, res);
    }
    res.writeHead(405);
    res.end("Method not allowed");
  });

  server.listen(PORT, HOST, () => {
    console.log(`LAB PWA em http://${HOST}:${PORT}`);
    console.log(`Backup dir: ${BACKUP_DIR}`);
    console.log(`Officialize dir: ${OFFICIALIZE_DIR}`);
  });
}

bootstrap().catch((err) => {
  console.error("Falha ao subir servidor:", err);
  process.exit(1);
});
