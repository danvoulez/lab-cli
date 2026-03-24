const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("labElectron", {
  isDesktop: true,
  process: async (mode, paths) => ipcRenderer.invoke("lab:process", { mode, paths }),
  choosePath: async () => ipcRenderer.invoke("lab:choose-path"),
});
