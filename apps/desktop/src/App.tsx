import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Onboarding } from "./components/onboarding";
import { SettingsSheet } from "./components/settings-sheet";
import { SessionsSheet } from "./components/sessions-sheet";
import { ModeBadge } from "./components/mode-badge";
import { ActivityFeed } from "./components/activity-feed";
import { StatusBar } from "./components/status-bar";
import { useRecordingStore } from "./stores/recording-store";
import { useOnboardingStore } from "./stores/onboarding-store";
import { useActivityStore } from "./stores/activity-store";
import { useStt } from "./hooks/use-stt";
import { useFunctionGemma } from "./hooks/use-functiongemma";
import { useActivityEvents } from "./hooks/use-activity-events";

function App() {
  const { isRecording, isListening, currentModel } = useRecordingStore();
  const { hasCompletedOnboarding } = useOnboardingStore();

  // Initialize hooks at app level to auto-load last used models on startup
  useStt();
  useFunctionGemma();
  useActivityEvents();

  // Start listening when model becomes available
  useEffect(() => {
    if (currentModel && !isListening && !isRecording) {
      console.log("[App] Model loaded, starting listen-only mode");
      const startListening = async () => {
        try {
          await invoke("plugin:gibberish-stt|reset_streaming_buffer");
          await invoke("plugin:gibberish-stt|stt_start_listening");
          await invoke("plugin:gibberish-recorder|start_listening", {
            sourceType: "combined_native",
          });
          useRecordingStore.getState().setIsListening(true);
        } catch (err) {
          console.error("Failed to start listening:", err);
        }
      };
      startListening();
    }
  }, [currentModel, isListening, isRecording]);

  const { searchQuery, setSearchQuery } = useActivityStore();
  const [showSettings, setShowSettings] = useState(false);
  const [showSessions, setShowSessions] = useState(false);

  if (!hasCompletedOnboarding) {
    return <Onboarding />;
  }

  return (
    <div className="flex flex-col h-screen" style={{ background: "var(--color-bg-primary)" }}>
      {/* Header with Mode Badge, Search, and Settings */}
      <header
        className="px-4 py-2 flex items-center gap-3"
        style={{ borderBottom: "1px solid var(--color-border)" }}
      >
        {/* Left: Mode Badge */}
        <ModeBadge />

        {/* Center: Search */}
        <div className="flex-1 max-w-xs mx-auto">
          <div className="relative">
            <svg
              className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4"
              style={{ color: "var(--color-text-quaternary)" }}
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <input
              type="text"
              placeholder="Search activities..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-8 pr-3 py-1.5 text-xs rounded-lg outline-none transition-colors"
              style={{
                background: "var(--color-bg-secondary)",
                border: "1px solid var(--color-border)",
                color: "var(--color-text-primary)",
              }}
            />
          </div>
        </div>

        {/* Right: Settings */}
        <button
          onClick={() => setShowSettings(true)}
          className="w-8 h-8 rounded-lg flex items-center justify-center transition-colors hover:bg-[var(--color-bg-secondary)]"
          title="Settings"
        >
          <svg
            className="w-5 h-5"
            style={{ color: "var(--color-text-tertiary)" }}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={1.5}
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z" />
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z" />
          </svg>
        </button>
      </header>

      {/* Main Content - Activity Feed */}
      <main className="flex-1 overflow-hidden flex flex-col">
        {!currentModel ? (
          <div className="flex-1 flex items-center justify-center p-6">
            <div className="text-center">
              <div
                className="w-20 h-20 rounded-full flex items-center justify-center mb-6 mx-auto"
                style={{ background: "var(--color-bg-secondary)" }}
              >
                <svg className="w-10 h-10" style={{ color: "var(--color-text-quaternary)" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
                </svg>
              </div>
              <h3 className="text-lg font-medium mb-2" style={{ color: "var(--color-text-primary)" }}>
                No Model Loaded
              </h3>
              <p className="mb-4" style={{ color: "var(--color-text-tertiary)" }}>
                Download a speech model to begin
              </p>
              <button onClick={() => setShowSettings(true)} className="btn-primary">
                Open Settings
              </button>
            </div>
          </div>
        ) : (
          <ActivityFeed />
        )}
      </main>

      {/* Status Bar */}
      <StatusBar />

      {/* Settings Sheet */}
      <SettingsSheet isOpen={showSettings} onClose={() => setShowSettings(false)} />

      {/* Sessions Sheet */}
      <SessionsSheet isOpen={showSessions} onClose={() => setShowSessions(false)} />
    </div>
  );
}

export default App;
