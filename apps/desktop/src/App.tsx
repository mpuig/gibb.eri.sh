import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TranscriptView } from "./components/transcript-view";
import { Onboarding } from "./components/onboarding";
import { SettingsSheet } from "./components/settings-sheet";
import { SessionsSheet } from "./components/sessions-sheet";
import { ModeBadge } from "./components/mode-badge";
import { ActionRouterPanel } from "./components/action-router-panel";
import { useRecordingStore } from "./stores/recording-store";
import { useOnboardingStore } from "./stores/onboarding-store";
import { useRecording } from "./hooks/use-recording";
import { useActionRouterStore } from "./stores/action-router-store";
import { useStt } from "./hooks/use-stt";
import { useFunctionGemma } from "./hooks/use-functiongemma";

function App() {
  const { isRecording, isListening, segments, currentModel } = useRecordingStore();
  const { hasCompletedOnboarding } = useOnboardingStore();
  const { isTranscribing, startRecording, stopRecording } = useRecording();

  // Initialize hooks at app level to auto-load last used models on startup
  useStt();
  useFunctionGemma();

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

  const { events, lastSearchResult, lastSearchError, lastNoMatch } = useActionRouterStore();
  const [showSettings, setShowSettings] = useState(false);
  const [showSessions, setShowSessions] = useState(false);
  const [copied, setCopied] = useState(false);

  const hasActionResult = lastSearchResult !== null || lastSearchError !== null || lastNoMatch !== null;
  const hasRouterActivity = events.length > 0 || hasActionResult;

  if (!hasCompletedOnboarding) {
    return <Onboarding />;
  }

  const handleRecordClick = () => {
    if (isRecording) {
      stopRecording();
    } else {
      startRecording();
    }
  };

  const handleCopy = async () => {
    const text = segments.map((s) => s.text).join("\n\n");
    if (text) {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const isDisabled = isTranscribing || (!isRecording && !currentModel);
  const hasTranscript = segments.length > 0;

  return (
    <div className="flex flex-col h-screen" style={{ background: "var(--color-bg-primary)" }}>
      {/* Header with Mode Badge and Listening Indicator */}
      <header
        className="px-4 py-2 flex items-center justify-between"
        style={{ borderBottom: "1px solid var(--color-border)" }}
      >
        <div className="flex items-center gap-2">
          <ModeBadge />
          {isListening && (
            <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs"
              style={{ background: "rgba(34, 197, 94, 0.15)", color: "rgb(34, 197, 94)" }}>
              <span className="w-1.5 h-1.5 rounded-full animate-pulse" style={{ background: "rgb(34, 197, 94)" }} />
              Listening
            </div>
          )}
        </div>
        <div className="text-xs" style={{ color: "var(--color-text-quaternary)" }}>
          gibberish
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 overflow-auto p-6">
        {!currentModel ? (
          <div className="empty-state h-full">
            <div
              className="w-20 h-20 rounded-full flex items-center justify-center mb-6"
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
        ) : isRecording || segments.length > 0 ? (
          // Recording mode: show transcript
          <>
            <TranscriptView segments={segments} />
            {hasActionResult && (
              <div className="mt-4">
                <ActionRouterPanel />
              </div>
            )}
          </>
        ) : isListening && hasRouterActivity ? (
          // Listen mode with activity: show action panel as main content
          <ActionRouterPanel />
        ) : isListening ? (
          // Listen mode, waiting for voice: show minimal indicator
          <div className="empty-state h-full">
            <p className="text-sm" style={{ color: "var(--color-text-quaternary)" }}>
              Say a voice command...
            </p>
          </div>
        ) : (
          // Not listening, no recording: prompt to record
          <div className="empty-state h-full">
            <div
              className="w-20 h-20 rounded-full flex items-center justify-center mb-6"
              style={{ background: "var(--color-bg-secondary)" }}
            >
              <svg
                className="w-10 h-10"
                style={{ color: "var(--color-text-quaternary)" }}
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={1.5}
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
              </svg>
            </div>
            <p style={{ color: "var(--color-text-tertiary)" }}>
              Press record to start transcribing
            </p>
          </div>
        )}
      </main>

      {/* Bottom Controls */}
      <footer
        className="px-6 py-4 flex items-center justify-between"
        style={{ background: "var(--color-bg-secondary)", borderTop: "1px solid var(--color-border)" }}
      >
        {/* Left: Settings */}
        <button
          onClick={() => setShowSettings(true)}
          className="w-10 h-10 rounded-full flex items-center justify-center transition-all hover:scale-105"
          style={{ background: "var(--color-bg-tertiary)" }}
          title="Settings"
        >
          <svg className="w-5 h-5" style={{ color: "var(--color-text-tertiary)" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z" />
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z" />
          </svg>
        </button>

        {/* Center: Record Button */}
        <button
          onClick={handleRecordClick}
          disabled={isDisabled}
          className="w-16 h-16 rounded-full flex items-center justify-center transition-all duration-200 hover:scale-105 shadow-lg"
          style={{
            background: isRecording
              ? "var(--color-danger)"
              : isDisabled
              ? "var(--color-bg-tertiary)"
              : "var(--color-danger)",
            cursor: isDisabled ? "not-allowed" : "pointer",
            opacity: isDisabled ? 0.5 : 1,
          }}
          title={isRecording ? "Stop Recording" : !currentModel ? "No Model Loaded" : "Start Recording"}
        >
          {isTranscribing ? (
            <svg className="animate-spin w-6 h-6" fill="none" viewBox="0 0 24 24" style={{ color: "white" }}>
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
            </svg>
          ) : isRecording ? (
            <div className="w-5 h-5 rounded-sm" style={{ background: "white" }} />
          ) : (
            <svg className="w-7 h-7" viewBox="0 0 24 24" fill={isDisabled ? "var(--color-text-quaternary)" : "white"}>
              <path d="M12 14a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v5a3 3 0 0 0 3 3z" />
              <path d="M19 11a1 1 0 1 0-2 0 5 5 0 0 1-10 0 1 1 0 1 0-2 0 7 7 0 0 0 6 6.92V20H8a1 1 0 1 0 0 2h8a1 1 0 1 0 0-2h-3v-2.08A7 7 0 0 0 19 11z" />
            </svg>
          )}
        </button>

        {/* Right: History & Copy */}
        <div className="flex items-center gap-2">
          {/* History Button */}
          <button
            onClick={() => setShowSessions(true)}
            className="w-10 h-10 rounded-full flex items-center justify-center transition-all hover:scale-105"
            style={{ background: "var(--color-bg-tertiary)" }}
            title="History"
          >
            <svg className="w-5 h-5" style={{ color: "var(--color-text-tertiary)" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
            </svg>
          </button>

          {/* Copy Button */}
          <button
            onClick={handleCopy}
            disabled={!hasTranscript}
            className="w-10 h-10 rounded-full flex items-center justify-center transition-all hover:scale-105"
            style={{
              background: copied ? "var(--color-success)" : "var(--color-bg-tertiary)",
              opacity: hasTranscript ? 1 : 0.3,
              cursor: hasTranscript ? "pointer" : "not-allowed",
            }}
            title={copied ? "Copied!" : "Copy Transcript"}
          >
            {copied ? (
              <svg className="w-5 h-5" style={{ color: "white" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            ) : (
              <svg className="w-5 h-5" style={{ color: "var(--color-text-tertiary)" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M15.666 3.888A2.25 2.25 0 0 0 13.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 0 1-.75.75H9a.75.75 0 0 1-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 0 1-2.25 2.25H6.75A2.25 2.25 0 0 1 4.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 0 1 1.927-.184" />
              </svg>
            )}
          </button>
        </div>
      </footer>

      {/* Settings Sheet */}
      <SettingsSheet isOpen={showSettings} onClose={() => setShowSettings(false)} />

      {/* Sessions Sheet */}
      <SessionsSheet isOpen={showSessions} onClose={() => setShowSessions(false)} />
    </div>
  );
}

export default App;
