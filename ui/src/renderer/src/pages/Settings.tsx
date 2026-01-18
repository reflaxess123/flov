import { useState, useEffect } from 'react'

interface Settings {
  language: string
}

export default function Settings() {
  const [settings, setSettings] = useState<Settings>({
    language: 'ru',
  })
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    window.electronAPI?.getSettings().then(setSettings)
  }, [])

  const handleSave = async () => {
    await window.electronAPI?.saveSettings(settings)
    setSaved(true)
    setTimeout(() => setSaved(false), 2000)
  }

  return (
    <div className="p-8 max-w-2xl">
      <h2 className="text-2xl font-semibold mb-6">Settings</h2>

      {/* Language */}
      <section className="mb-8">
        <h3 className="text-lg font-medium mb-4 text-gray-300">Language</h3>
        <select
          value={settings.language}
          onChange={e => setSettings(s => ({ ...s, language: e.target.value }))}
          className="w-full px-4 py-3 rounded-lg bg-white/5 border border-white/10 text-white focus:border-purple-500 focus:outline-none"
        >
          <option value="ru">Russian</option>
          <option value="en">English</option>
          <option value="auto">Auto-detect</option>
        </select>
      </section>

      {/* Save Button */}
      <button
        onClick={handleSave}
        className={`px-6 py-3 rounded-lg font-medium transition-all ${
          saved
            ? 'bg-green-600 text-white'
            : 'bg-purple-600 hover:bg-purple-500 text-white'
        }`}
      >
        {saved ? 'Saved' : 'Save Settings'}
      </button>
    </div>
  )
}
