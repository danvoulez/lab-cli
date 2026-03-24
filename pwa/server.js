const http = require("http");
const fs = require("fs");
const fsp = fs.promises;
const path = require("path");
const { spawn } = require("child_process");

const PORT = Number(process.env.PORT || 4319);
const HOST = "127.0.0.1";

const PWA_DIR = __dirname;
const REPO_DIR = path.resolve(PWA_DIR, "..");
const PUBLIC_DIR = path.join(PWA_DIR, "public");
const LAB_BIN = path.join(REPO_DIR, "target", "debug", "lab");

const BACKUP_DIR = "/Users/ubl-ops/BACKUP-FEITO";
const OFFICIALIZE_DIR = "/Users/ubl-ops/officialize";

async function ensureDirs() {
  await fsp.mkdir(BACKUP_DIR, { recursive: true });
  await fsp.mkdir(OFFICIALIZE_DIR, { recursive: true });
}

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

async function exists(p) {
  try {
    await fsp.access(p, fs.constants.F_OK);
    return true;
  } catch {
    return false;
  }
}

function runCommand(command, args, cwd) {
  return new Promise((resolve) => {
    const child = spawn(command, args, { cwd, env: process.env });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => {
      stdout += d.toString();
    });
    child.stderr.on("data", (d) => {
      stderr += d.toString();
    });
    child.on("close", (code) => {
      resolve({ code: code ?? 1, stdout, stderr });
    });
  });
}

async function runLab(mode, filePath) {
  const labArgs = [mode, filePath, "--normalizer", "local-ollama"];

  if (mode === "backup" && process.env.DRIVE_FOLDER_ID) {
    labArgs.push("--drive-folder-id", process.env.DRIVE_FOLDER_ID);
  }

  if (mode === "officialize") {
    if (process.env.SUPABASE_BUCKET) {
      labArgs.push("--supabase-bucket", process.env.SUPABASE_BUCKET);
    }
    if (process.env.SUPABASE_PATH_PREFIX) {
      labArgs.push("--supabase-path-prefix", process.env.SUPABASE_PATH_PREFIX);
    }
    if (process.env.SUPABASE_TABLE) {
      labArgs.push("--supabase-table", process.env.SUPABASE_TABLE);
    } else {
      labArgs.push("--supabase-table", "LAB-OFFICIAL-INDEX");
    }
  }

  if (await exists(LAB_BIN)) {
    return runCommand(LAB_BIN, labArgs, REPO_DIR);
  }

  const cargoArgs = [
    "run",
    "--manifest-path",
    path.join(REPO_DIR, "Cargo.toml"),
    "--",
    ...labArgs,
  ];
  return runCommand("cargo", cargoArgs, REPO_DIR);
}

async function movePath(sourcePath, targetDir, isDirectory) {
  const base = path.basename(sourcePath);
  const parsed = path.parse(base);
  let candidate = path.join(targetDir, base);
  let idx = 1;

  while (await exists(candidate)) {
    const suffix = isDirectory ? `-${idx}` : `-${idx}${parsed.ext}`;
    const name = isDirectory ? base : parsed.name;
    candidate = path.join(targetDir, `${name}${suffix}`);
    idx += 1;
  }

  try {
    await fsp.rename(sourcePath, candidate);
  } catch (err) {
    if (err.code !== "EXDEV") throw err;
    if (isDirectory) {
      await fsp.cp(sourcePath, candidate, { recursive: true });
      await fsp.rm(sourcePath, { recursive: true, force: true });
    } else {
      await fsp.copyFile(sourcePath, candidate);
      await fsp.unlink(sourcePath);
    }
  }
  return candidate;
}

function isFileUri(text) {
  return typeof text === "string" && text.startsWith("file://");
}

function decodeFileUri(uri) {
  const u = new URL(uri.trim());
  return decodeURIComponent(u.pathname);
}

async function handleProcess(req, res) {
  try {
    const body = await parseBody(req);
    const mode = body.mode;
    const rawPaths = Array.isArray(body.paths) ? body.paths : [];

    if (!["backup", "officialize"].includes(mode)) {
      return json(res, 400, { error: "mode invalido" });
    }
    if (rawPaths.length === 0) {
      return json(res, 400, { error: "nenhum arquivo informado" });
    }

    const paths = rawPaths
      .map((p) => String(p || "").trim())
      .filter(Boolean)
      .map((p) => (isFileUri(p) ? decodeFileUri(p) : p));

    const targetDir = mode === "backup" ? BACKUP_DIR : OFFICIALIZE_DIR;
    const results = [];

    for (const filePath of paths) {
      const abs = path.resolve(filePath);
      let stat;
      try {
        stat = await fsp.stat(abs);
        if (!stat.isFile() && !stat.isDirectory()) {
          results.push({ file: abs, ok: false, error: "item invalido (esperado arquivo ou pasta)" });
          continue;
        }
      } catch {
        results.push({ file: abs, ok: false, error: "arquivo/pasta nao encontrado" });
        continue;
      }

      const cmdResult = await runLab(mode, abs);
      if (cmdResult.code !== 0) {
        results.push({
          file: abs,
          ok: false,
          error: "falha ao executar CLI",
          stdout: cmdResult.stdout,
          stderr: cmdResult.stderr,
        });
        continue;
      }

      try {
        const movedTo = await movePath(abs, targetDir, stat.isDirectory());
        results.push({
          file: abs,
          ok: true,
          moved_to: movedTo,
          stdout: cmdResult.stdout,
          stderr: cmdResult.stderr,
        });
      } catch (err) {
        results.push({
          file: abs,
          ok: false,
          error: `CLI ok, mas falha ao mover arquivo: ${err.message}`,
          stdout: cmdResult.stdout,
          stderr: cmdResult.stderr,
        });
      }
    }

    return json(res, 200, { mode, results });
  } catch (err) {
    return json(res, 500, { error: err.message });
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
