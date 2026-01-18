import { app, BrowserWindow, Tray, Menu, nativeImage, ipcMain, screen } from 'electron'
import { spawn, ChildProcess } from 'child_process'
import * as path from 'path'

let mainWindow: BrowserWindow | null = null
let overlayWindow: BrowserWindow | null = null
let tray: Tray | null = null
let rustBackend: ChildProcess | null = null

// App state
let isRecording = false
let isQuitting = false

function createMainWindow() {
  mainWindow = new BrowserWindow({
    width: 800,
    height: 600,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, '../preload/index.cjs'),
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  if (!app.isPackaged && process.env['ELECTRON_RENDERER_URL']) {
    mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL'])
  } else {
    mainWindow.loadFile(path.join(__dirname, '../renderer/index.html'))
  }

  mainWindow.on('close', (e) => {
    if (!isQuitting) {
      e.preventDefault()
      mainWindow?.hide()
    }
  })
}

function createOverlayWindow() {
  overlayWindow = new BrowserWindow({
    width: 200,
    height: 60,
    frame: false,
    transparent: true,
    resizable: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    focusable: false,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, '../preload/index.cjs'),
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  // Disable mouse events on the window (click-through)
  overlayWindow.setIgnoreMouseEvents(true)

  if (!app.isPackaged && process.env['ELECTRON_RENDERER_URL']) {
    overlayWindow.loadURL(process.env['ELECTRON_RENDERER_URL'] + '#/overlay')
  } else {
    overlayWindow.loadFile(path.join(__dirname, '../renderer/index.html'), { hash: 'overlay' })
  }
}

function showOverlay() {
  if (!overlayWindow) return

  // Get cursor position
  const cursorPos = screen.getCursorScreenPoint()

  // Position overlay near cursor
  overlayWindow.setPosition(cursorPos.x + 20, cursorPos.y - 30)
  overlayWindow.show()
}

function hideOverlay() {
  overlayWindow?.hide()
}

function createTray() {
  // Create a simple colored icon
  const iconSize = 16
  const icon = nativeImage.createEmpty()

  tray = new Tray(icon)

  updateTrayIcon(false)
  updateTrayMenu()

  tray.on('click', () => {
    mainWindow?.show()
  })
}

function updateTrayIcon(recording: boolean) {
  if (!tray) return

  // Create icon with canvas-like approach
  const size = 16
  const canvas = Buffer.alloc(size * size * 4)

  const color = recording ? [0, 200, 0, 255] : [200, 50, 50, 255] // Green or Red

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      // Create a circle
      const cx = size / 2
      const cy = size / 2
      const r = size / 2 - 1
      const dist = Math.sqrt((x - cx) ** 2 + (y - cy) ** 2)

      const idx = (y * size + x) * 4
      if (dist <= r) {
        canvas[idx] = color[0]     // R
        canvas[idx + 1] = color[1] // G
        canvas[idx + 2] = color[2] // B
        canvas[idx + 3] = color[3] // A
      } else {
        canvas[idx + 3] = 0 // Transparent
      }
    }
  }

  const icon = nativeImage.createFromBuffer(canvas, { width: size, height: size })
  tray.setImage(icon)
}

function updateTrayMenu() {
  if (!tray) return

  const contextMenu = Menu.buildFromTemplate([
    { label: 'Open Settings', click: () => mainWindow?.show() },
    { type: 'separator' },
    { label: 'Quit', click: () => {
      isQuitting = true
      rustBackend?.kill()
      tray?.destroy()
      app.exit(0)
    }}
  ])

  tray.setContextMenu(contextMenu)
}

function startRustBackend() {
  const backendPath = !app.isPackaged
    ? path.join(__dirname, '../../../target/release/flov.exe')
    : path.join(process.resourcesPath, 'flov.exe')

  rustBackend = spawn(backendPath, ['--ipc'], {
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true
  })

  rustBackend.stdout?.on('data', (data) => {
    const messages = data.toString().split('\n').filter((l: string) => l.trim())
    for (const msg of messages) {
      try {
        const parsed = JSON.parse(msg)
        handleBackendMessage(parsed)
      } catch (e) {
        console.log('Backend:', msg)
      }
    }
  })

  rustBackend.stderr?.on('data', (data) => {
    console.error('Backend error:', data.toString())
  })

  rustBackend.on('close', (code) => {
    console.log('Backend exited with code:', code)
  })
}

function sendToBackend(message: object) {
  if (rustBackend?.stdin) {
    rustBackend.stdin.write(JSON.stringify(message) + '\n')
  }
}

function handleBackendMessage(msg: any) {
  switch (msg.type) {
    case 'recording_started':
      isRecording = true
      updateTrayIcon(true)
      showOverlay()
      overlayWindow?.webContents.send('recording-state', { recording: true })
      break

    case 'recording_stopped':
      isRecording = false
      updateTrayIcon(false)
      overlayWindow?.webContents.send('recording-state', { recording: false, transcribing: true })
      break

    case 'transcribing':
      // Keep overlay visible with loading state
      break

    case 'spectrum':
      // Send frequency spectrum array (20 values, 0-1)
      overlayWindow?.webContents.send('spectrum', msg.values)
      break

    case 'transcription':
      mainWindow?.webContents.send('transcription', msg.text)
      hideOverlay()
      overlayWindow?.webContents.send('recording-state', { recording: false, transcribing: false })
      break

    case 'error':
      console.error('Backend error:', msg.message)
      hideOverlay()
      overlayWindow?.webContents.send('recording-state', { recording: false, transcribing: false })
      break
  }
}

// IPC handlers
ipcMain.handle('get-settings', async () => {
  // TODO: Load from config file
  return { whisperModel: 'medium', language: 'ru' }
})

ipcMain.handle('save-settings', async (_, settings) => {
  sendToBackend({ type: 'settings', ...settings })
})

app.whenReady().then(() => {
  createMainWindow()
  createOverlayWindow()
  createTray()
  startRustBackend()
})

app.on('window-all-closed', () => {
  // Don't quit on window close - keep in tray
})

app.on('activate', () => {
  mainWindow?.show()
})

app.on('before-quit', () => {
  rustBackend?.kill()
})
