import { contextBridge, ipcRenderer } from 'electron'

// Expose protected methods to renderer
contextBridge.exposeInMainWorld('electronAPI', {
  // Settings
  getSettings: () => ipcRenderer.invoke('get-settings'),
  saveSettings: (settings: any) => ipcRenderer.invoke('save-settings', settings),

  // Events from main process
  onRecordingState: (callback: (data: { recording: boolean, transcribing?: boolean }) => void) => {
    ipcRenderer.on('recording-state', (_, data) => callback(data))
  },

  onSpectrum: (callback: (values: number[]) => void) => {
    ipcRenderer.on('spectrum', (_, values) => callback(values))
  },

  onTranscription: (callback: (text: string) => void) => {
    ipcRenderer.on('transcription', (_, text) => callback(text))
  },

  // Cleanup
  removeAllListeners: (channel: string) => {
    ipcRenderer.removeAllListeners(channel)
  }
})
