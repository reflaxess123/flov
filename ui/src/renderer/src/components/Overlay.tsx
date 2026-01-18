import { useEffect, useState } from 'react'

export default function Overlay() {
  const [isRecording, setIsRecording] = useState(false)
  const [isTranscribing, setIsTranscribing] = useState(false)
  const [spectrum, setSpectrum] = useState<number[]>(new Array(20).fill(0.1))

  useEffect(() => {
    // Listen for recording state changes
    window.electronAPI?.onRecordingState((data) => {
      setIsRecording(data.recording)
      setIsTranscribing(data.transcribing ?? false)
      if (!data.recording && !data.transcribing) {
        // Reset spectrum when done
        setSpectrum(new Array(20).fill(0.1))
      }
    })

    // Listen for frequency spectrum from real microphone
    window.electronAPI?.onSpectrum((values: number[]) => {
      setSpectrum(values)
    })

    return () => {
      window.electronAPI?.removeAllListeners('recording-state')
      window.electronAPI?.removeAllListeners('spectrum')
    }
  }, [])

  if (!isRecording && !isTranscribing) return null

  return (
    <div className="w-full h-full flex items-center justify-center">
      <div className="flex items-center justify-center h-[40px] bg-[#1a1a2e]/90 backdrop-blur-sm px-4 py-2 rounded-full border border-purple-500/30 shadow-lg shadow-purple-500/20">
        {isTranscribing ? (
          // Loading spinner
          <div className="flex items-center gap-2">
            <div className="w-5 h-5 border-2 border-purple-400 border-t-transparent rounded-full animate-spin" />
          </div>
        ) : (
          // Frequency spectrum
          <div className="flex items-end gap-[3px] h-full">
            {spectrum.map((val, i) => (
              <div
                key={i}
                className="w-[4px] rounded-full bg-gradient-to-t from-purple-500 to-purple-300"
                style={{
                  height: `${Math.max(4, val * 28)}px`,
                  opacity: 0.6 + val * 0.4,
                  transition: 'height 30ms ease-out'
                }}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
