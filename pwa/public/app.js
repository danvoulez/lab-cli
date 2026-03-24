const track = document.getElementById("track");
const leftArrow = document.getElementById("leftArrow");
const rightArrow = document.getElementById("rightArrow");

const logBackup = document.getElementById("logBackup");
const logOfficialize = document.getElementById("logOfficialize");
const pasteBackup = document.getElementById("pasteBackup");
const pasteOfficialize = document.getElementById("pasteOfficialize");

let screenIndex = 0;

function setScreen(nextIndex) {
  screenIndex = Math.max(0, Math.min(1, nextIndex));
  track.style.transform = `translateX(-${screenIndex * 50}%)`;
}

leftArrow.addEventListener("click", () => setScreen(screenIndex - 1));
rightArrow.addEventListener("click", () => setScreen(screenIndex + 1));

function appendLog(mode, text) {
  const el = mode === "backup" ? logBackup : logOfficialize;
  const now = new Date().toLocaleTimeString();
  el.textContent = `[${now}] ${text}\n` + el.textContent;
}

function readUriListFromEvent(event) {
  const uriList = event.dataTransfer.getData("text/uri-list");
  if (!uriList) return [];
  return uriList
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"))
    .map((uri) => decodeFileUri(uri))
    .filter(Boolean);
}

function decodeFileUri(uri) {
  try {
    if (!uri.startsWith("file://")) return null;
    const u = new URL(uri);
    return decodeURIComponent(u.pathname);
  } catch {
    return null;
  }
}

function normalizePossiblePath(text) {
  const value = String(text || "").trim().replace(/^"+|"+$/g, "");
  if (!value) return null;
  if (value.startsWith("file://")) return decodeFileUri(value);
  if (value.startsWith("/")) return value;
  return null;
}

function readTextPlainPaths(event) {
  const raw = event.dataTransfer.getData("text/plain");
  if (!raw) return [];
  return raw
    .split("\n")
    .map((line) => normalizePossiblePath(line))
    .filter(Boolean);
}

async function readItemStringPaths(event) {
  const items = Array.from(event.dataTransfer.items || []);
  const stringItems = items.filter((it) => it.kind === "string");
  const out = [];

  for (const item of stringItems) {
    const value = await new Promise((resolve) => {
      try {
        item.getAsString((s) => resolve(s || ""));
      } catch {
        resolve("");
      }
    });

    const lines = String(value || "")
      .split("\n")
      .map((line) => normalizePossiblePath(line))
      .filter(Boolean);
    out.push(...lines);
  }

  return out;
}

function readPathsFallbackFromFiles(event) {
  const files = Array.from(event.dataTransfer.files || []);
  return files.map((f) => f.path).filter(Boolean);
}

async function processDrop(mode, paths) {
  appendLog(mode, `Enviando ${paths.length} item(ns) (arquivo/pasta) para ${mode}...`);

  let data;
  if (window.labElectron?.isDesktop) {
    data = await window.labElectron.process(mode, paths);
  } else {
    const resp = await fetch("/api/process", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ mode, paths }),
    });
    data = await resp.json();
    if (!resp.ok) {
      appendLog(mode, `Erro API: ${data.error || "desconhecido"}`);
      return;
    }
  }

  if (data?.error) {
    appendLog(mode, `Erro API: ${data.error || "desconhecido"}`);
    return;
  }

  for (const r of data.results || []) {
    if (r.ok) {
      appendLog(mode, `OK: ${r.file} -> ${r.moved_to}`);
    } else {
      appendLog(mode, `ERRO: ${r.file} -> ${r.error || "falha"}`);
    }
  }
}

function bindDropzone(zone) {
  const mode = zone.dataset.mode;
  zone.addEventListener("dragover", (event) => {
    event.preventDefault();
    zone.classList.add("drag");
  });

  zone.addEventListener("dragleave", () => {
    zone.classList.remove("drag");
  });

  zone.addEventListener("drop", async (event) => {
    event.preventDefault();
    zone.classList.remove("drag");

    const itemStringPaths = await readItemStringPaths(event);
    const uriPaths = readUriListFromEvent(event);
    const textPaths = readTextPlainPaths(event);
    const fallbackPaths = readPathsFallbackFromFiles(event);
    const paths = [...new Set([...itemStringPaths, ...uriPaths, ...textPaths, ...fallbackPaths])];

    if (paths.length === 0) {
      appendLog(mode, "Nao consegui ler caminho absoluto do arquivo no drop.");
      appendLog(
        mode,
        "Use botao COLAR CAMINHO e cole um path absoluto (ex: /Users/voce/projeto)."
      );
      return;
    }

    try {
      await processDrop(mode, paths);
    } catch (err) {
      appendLog(mode, `Falha geral: ${err.message}`);
    }
  });
}

bindDropzone(document.getElementById("dropBackup"));
bindDropzone(document.getElementById("dropOfficialize"));

function bindPasteButton(btn, mode) {
  if (!btn) return;
  btn.addEventListener("click", async () => {
    let normalized = null;

    if (window.labElectron?.isDesktop && window.labElectron?.choosePath) {
      const chosen = await window.labElectron.choosePath();
      const first = Array.isArray(chosen) ? chosen[0] : null;
      normalized = normalizePossiblePath(first || "");
    } else {
      const value = window.prompt(
        "Cole caminho absoluto de arquivo ou pasta (ex: /Users/voce/projeto):"
      );
      normalized = normalizePossiblePath(value || "");
    }

    if (!normalized) {
      appendLog(mode, "Caminho invalido. Informe path absoluto.");
      return;
    }
    try {
      await processDrop(mode, [normalized]);
    } catch (err) {
      appendLog(mode, `Falha geral: ${err.message}`);
    }
  });
}

bindPasteButton(pasteBackup, "backup");
bindPasteButton(pasteOfficialize, "officialize");

if (window.labElectron?.isDesktop) {
  if (pasteBackup) pasteBackup.textContent = "ESCOLHER FINDER";
  if (pasteOfficialize) pasteOfficialize.textContent = "ESCOLHER FINDER";
}

if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("/sw.js").catch(() => {});
  });
}
