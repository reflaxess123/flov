import { useEffect, useState } from 'react'
import { HashRouter, Routes, Route } from 'react-router-dom'
import Settings from './pages/Settings'
import History from './pages/History'
import Overlay from './components/Overlay'
import Sidebar from './components/Sidebar'

// Type declarations for electron API
declare global {
  interface Window {
    electronAPI?: {
      getSettings: () => Promise<any>
      saveSettings: (settings: any) => Promise<void>
      onRecordingState: (callback: (data: { recording: boolean }) => void) => void
      onAmplitude: (callback: (value: number) => void) => void
      onTranscription: (callback: (text: string) => void) => void
      removeAllListeners: (channel: string) => void
    }
  }
}

function MainLayout() {
  const [history, setHistory] = useState<Array<{ id: number; text: string; timestamp: Date }>>([])

  useEffect(() => {
    const handleTranscription = (text: string) => {
      setHistory(prev => [
        { id: Date.now(), text, timestamp: new Date() },
        ...prev
      ])
    }

    window.electronAPI?.onTranscription(handleTranscription)

    return () => {
      window.electronAPI?.removeAllListeners('transcription')
    }
  }, [])

  return (
    <div className="flex h-screen bg-[#0f0f1a] text-white">
      <Sidebar />
      <main className="flex-1 overflow-auto">
        <Routes>
          <Route path="/" element={<Settings />} />
          <Route path="/history" element={<History items={history} />} />
        </Routes>
      </main>
    </div>
  )
}

function App() {
  return (
    <HashRouter>
      <Routes>
        <Route path="/overlay" element={<Overlay />} />
        <Route path="/*" element={<MainLayout />} />
      </Routes>
    </HashRouter>
  )
}

export default App
