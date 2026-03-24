const fs = require("fs");
const fsp = fs.promises;
const path = require("path");
const { spawn } = require("child_process");

const PWA_DIR = __dirname;
const REPO_DIR = path.resolve(PWA_DIR, "..");
const LAB_BIN = path.join(REPO_DIR, "target", "debug", "lab");

const BACKUP_DIR = "/Users/ubl-ops/BACKUP-FEITO";
const OFFICIALIZE_DIR = "/Users/ubl-ops/officialize";

async function ensureDirs() {
  await fsp.mkdir(BACKUP_DIR, { recursive: true });
  await fsp.mkdir(OFFICIALIZE_DIR, { recursive: true });
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

async function runLab(mode, inputPath) {
  const labArgs = [mode, inputPath, "--normalizer", "local-ollama"];

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

async function processPaths(mode, rawPaths) {
  if (!["backup", "officialize"].includes(mode)) {
    throw new Error("mode invalido");
  }
  const rawList = Array.isArray(rawPaths) ? rawPaths : [];
  if (rawList.length === 0) {
    throw new Error("nenhum arquivo informado");
  }

  const paths = rawList
    .map((p) => String(p || "").trim())
    .filter(Boolean)
    .map((p) => (isFileUri(p) ? decodeFileUri(p) : p));

  const targetDir = mode === "backup" ? BACKUP_DIR : OFFICIALIZE_DIR;
  const results = [];

  for (const itemPath of paths) {
    const abs = path.resolve(itemPath);
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

  return { mode, results };
}

module.exports = {
  BACKUP_DIR,
  OFFICIALIZE_DIR,
  ensureDirs,
  processPaths,
};
