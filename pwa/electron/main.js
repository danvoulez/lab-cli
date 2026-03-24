const path = require("path");
const { app, BrowserWindow, ipcMain, dialog } = require("electron");

const { BACKUP_DIR, OFFICIALIZE_DIR, ensureDirs, processPaths } = require("../processor");

function createWindow() {
  const win = new BrowserWindow({
    width: 1280,
    height: 860,
    minWidth: 980,
    minHeight: 680,
    backgroundColor: "#3c3c3c",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  win.loadFile(path.join(__dirname, "..", "public", "index.html"));
}

ipcMain.handle("lab:process", async (_event, payload) => {
  const mode = payload?.mode;
  const paths = payload?.paths;
  return processPaths(mode, paths);
});

ipcMain.handle("lab:choose-path", async () => {
  const result = await dialog.showOpenDialog({
    title: "Escolher arquivo ou pasta",
    properties: ["openFile", "openDirectory", "multiSelections"],
  });
  if (result.canceled) return [];
  return result.filePaths || [];
});

app.whenReady().then(async () => {
  await ensureDirs();
  createWindow();
  console.log("LAB Desktop (Electron) iniciado");
  console.log(`Backup dir: ${BACKUP_DIR}`);
  console.log(`Officialize dir: ${OFFICIALIZE_DIR}`);

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
