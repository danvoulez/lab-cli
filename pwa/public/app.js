const track = document.getElementById("track");
const leftArrow = document.getElementById("leftArrow");
const rightArrow = document.getElementById("rightArrow");

const logBackup = document.getElementById("logBackup");
const logOfficialize = document.getElementById("logOfficialize");

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

function readPathsFallbackFromFiles(event) {
  const files = Array.from(event.dataTransfer.files || []);
  return files.map((f) => f.path).filter(Boolean);
}

async function processDrop(mode, paths) {
  appendLog(mode, `Enviando ${paths.length} item(ns) (arquivo/pasta) para ${mode}...`);

  const resp = await fetch("/api/process", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ mode, paths }),
  });
  const data = await resp.json();

  if (!resp.ok) {
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

    const uriPaths = readUriListFromEvent(event);
    const fallbackPaths = readPathsFallbackFromFiles(event);
    const paths = [...new Set([...uriPaths, ...fallbackPaths])];

    if (paths.length === 0) {
      appendLog(mode, "Nao consegui ler caminho absoluto do arquivo no drop.");
      appendLog(
        mode,
        "Tente arrastar do Finder em navegador Chromium, ou use ambiente que exponha file:// no drop."
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

if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("/sw.js").catch(() => {});
  });
}
