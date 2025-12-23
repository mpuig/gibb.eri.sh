import { useState, useCallback, useMemo } from "react";
import { TranscriptView } from "./components/transcript-view";
import { ActionRouterPanel } from "./components/action-router-panel";
import { RecordingControls } from "./components/recording-controls";
import { Settings } from "./components/model-settings";
import { ExportMenu } from "./components/export-menu";
import { SessionList } from "./components/session-list";
import { SessionViewer } from "./components/session-viewer";
import { Onboarding } from "./components/onboarding";
import { useRecordingStore, type TranscriptSegment } from "./stores/recording-store";
import { useOnboardingStore } from "./stores/onboarding-store";
import { useRecording } from "./hooks/use-recording";
import { useDetect, InstalledApp } from "./hooks/use-detect";
import { useSessionsStore } from "./stores/sessions-store";

type Tab = "record" | "sessions" | "settings";

const MicIcon = () => (
  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
  </svg>
);

const FolderIcon = () => (
  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
  </svg>
);

const GearIcon = () => (
  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
    <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
  </svg>
);

function App() {
  const { isRecording, segments, currentModel } = useRecordingStore();
  const { hasCompletedOnboarding } = useOnboardingStore();
  const { currentSession } = useSessionsStore();
  const { startRecording, stopRecording } = useRecording();
  const [activeTab, setActiveTab] = useState<Tab>("record");
  const [viewingSessionId, setViewingSessionId] = useState<string | null>(null);
  const [autoRecordNotification, setAutoRecordNotification] = useState<string | null>(null);
  const [autoRecordedMeetingApp, setAutoRecordedMeetingApp] = useState<string | null>(null);

  // Convert session segments to export format
  const sessionExportSegments = useMemo<TranscriptSegment[]>(() => {
    if (!currentSession) return [];
    return currentSession.segments.map((s) => ({
      id: s.id,
      text: s.text,
      startMs: s.startMs,
      endMs: s.endMs,
      speaker: s.speaker ?? undefined,
      isFinal: true,
    }));
  }, [currentSession]);

  // Helper to check if an app is a meeting app
  const isMeetingApp = useCallback((app: InstalledApp) => {
    const meetingBundleIds = [
      "us.zoom.xos",
      "Cisco-Systems.Spark",
      "com.microsoft.teams",
      "com.microsoft.teams2",
      "com.discord.Discord",
      "com.slack.Slack",
    ];
    return (
      meetingBundleIds.includes(app.id) ||
      app.name.toLowerCase().includes("zoom") ||
      app.name.toLowerCase().includes("teams") ||
      app.name.toLowerCase().includes("meet") ||
      app.name.toLowerCase().includes("webex")
    );
  }, []);

  // Auto-record when meeting apps are detected
  const handleMicStarted = useCallback((apps: InstalledApp[], _key: string) => {
    if (isRecording || !currentModel) return;

    const meetingApps = apps.filter(isMeetingApp);

    if (meetingApps.length > 0) {
      const appNames = meetingApps.map(a => a.name).join(", ");
      const firstMeetingAppId = meetingApps[0].id;
      console.log("Meeting detected, will auto-start recording in 2s:", appNames);
      setAutoRecordNotification(`Auto-recording: ${appNames}`);
      setAutoRecordedMeetingApp(firstMeetingAppId);

      // Delay recording start to let the meeting app initialize its audio
      setTimeout(() => {
        // Check again that we're not already recording
        if (!useRecordingStore.getState().isRecording) {
          startRecording();
        }
      }, 2000);

      // Clear notification after 5 seconds
      setTimeout(() => setAutoRecordNotification(null), 5000);
    }
  }, [isRecording, currentModel, startRecording, isMeetingApp]);

  // Auto-stop recording when the meeting app that triggered auto-record stops
  const handleMicStopped = useCallback((apps: InstalledApp[]) => {
    // Only auto-stop if we auto-started for a meeting app
    if (!autoRecordedMeetingApp) return;

    // Check if the meeting app that triggered auto-record is in the stopped list
    const stoppedMeetingApp = apps.find(app => app.id === autoRecordedMeetingApp);

    if (stoppedMeetingApp) {
      console.log("Meeting ended, auto-stopping recording:", stoppedMeetingApp.name);
      setAutoRecordNotification(`Meeting ended: ${stoppedMeetingApp.name}`);
      setAutoRecordedMeetingApp(null);

      // Stop recording if we're currently recording
      if (useRecordingStore.getState().isRecording) {
        stopRecording();
      }

      // Clear notification after 5 seconds
      setTimeout(() => setAutoRecordNotification(null), 5000);
    }
  }, [autoRecordedMeetingApp, stopRecording]);

  useDetect({
    onMicStarted: handleMicStarted,
    onMicStopped: handleMicStopped,
  });

  if (!hasCompletedOnboarding) {
    return <Onboarding />;
  }

  const handleSelectSession = (id: string) => {
    setViewingSessionId(id);
  };

  const handleBackFromSession = () => {
    setViewingSessionId(null);
  };

  const tabs = [
    { id: "record" as Tab, label: "Record", icon: MicIcon },
    { id: "sessions" as Tab, label: "Sessions", icon: FolderIcon },
    { id: "settings" as Tab, label: "Settings", icon: GearIcon },
  ];

  return (
    <div className="flex flex-col h-screen" style={{ background: "var(--color-bg-primary)" }}>
      {/* Header */}
      <header className="glass border-b" style={{ borderColor: "var(--color-border)" }}>
        <div className="flex items-center justify-center px-4 py-3">
          {/* Tab Navigation - Centered */}
          <nav className="tab-nav">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => {
                  setActiveTab(tab.id);
                  setViewingSessionId(null);
                }}
                className={`tab-item flex items-center gap-2 ${
                  activeTab === tab.id ? "tab-active" : ""
                }`}
              >
                <tab.icon />
                {tab.label}
              </button>
            ))}
          </nav>
        </div>
      </header>

      {/* Auto-record notification */}
      {autoRecordNotification && (
        <div
          className="mx-4 mt-2 px-4 py-2 rounded-lg flex items-center gap-2 animate-in"
          style={{
            background: "var(--color-success)",
            color: "white",
          }}
        >
          <span className="relative flex h-2 w-2">
            <span
              className="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75"
              style={{ background: "white" }}
            />
            <span className="relative inline-flex rounded-full h-2 w-2" style={{ background: "white" }} />
          </span>
          <span className="text-sm font-medium">{autoRecordNotification}</span>
        </div>
      )}

      {/* Main Content */}
      <main className="flex-1 overflow-auto">
        {activeTab === "record" && (
          <div className="h-full animate-in flex flex-col">
            {/* Record Panel Toolbar */}
            <div className="flex items-center justify-center px-4 py-3 border-b" style={{ borderColor: "var(--color-border)" }}>
              <RecordingControls onRecordingStart={() => setActiveTab("record")} />
            </div>

            {/* Content Area */}
            <div className="flex-1 overflow-auto">
              {segments.length === 0 && !isRecording ? (
                <div className="empty-state h-full">
                  <div
                    className="w-20 h-20 rounded-full flex items-center justify-center mb-6"
                    style={{ background: "var(--color-bg-secondary)" }}
                  >
                    <svg className="w-10 h-10" style={{ color: "var(--color-text-quaternary)" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
                    </svg>
                  </div>
                  {currentModel ? (
                    <>
                      <h3 className="text-lg font-medium mb-2" style={{ color: "var(--color-text-primary)" }}>
                        Ready to Record
                      </h3>
                      <p style={{ color: "var(--color-text-tertiary)" }}>
                        Click the record button to start transcribing
                      </p>
                    </>
                  ) : (
                    <>
                      <h3 className="text-lg font-medium mb-2" style={{ color: "var(--color-text-primary)" }}>
                        No Model Loaded
                      </h3>
                      <p className="mb-4" style={{ color: "var(--color-text-tertiary)" }}>
                        Download and load a speech model to begin
                      </p>
                      <button
                        onClick={() => setActiveTab("settings")}
                        className="btn-primary"
                      >
                        Go to Settings
                      </button>
                    </>
                  )}
                </div>
              ) : (
                <div className="p-4 space-y-4">
                  <TranscriptView segments={segments} />
                  <ActionRouterPanel />
                </div>
              )}
            </div>
          </div>
        )}

        {activeTab === "sessions" && (
          <div className="h-full animate-in">
            {viewingSessionId ? (
              <SessionViewer
                sessionId={viewingSessionId}
                onBack={handleBackFromSession}
              />
            ) : (
              <SessionList onSelectSession={handleSelectSession} />
            )}
          </div>
        )}

        {activeTab === "settings" && (
          <div className="animate-in">
            <Settings />
          </div>
        )}
      </main>

      {/* Status Bar */}
      <footer
        className="px-4 py-2 border-t flex justify-between items-center"
        style={{
          borderColor: "var(--color-border)",
          background: "var(--color-bg-secondary)",
          fontSize: "0.75rem"
        }}
      >
        <div className="flex items-center gap-2">
          {isRecording ? (
            <>
              <span className="relative flex h-2 w-2">
                <span
                  className="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75"
                  style={{ background: "var(--color-danger)" }}
                />
                <span
                  className="relative inline-flex rounded-full h-2 w-2"
                  style={{ background: "var(--color-danger)" }}
                />
              </span>
              <span style={{ color: "var(--color-danger)" }}>Recording</span>
            </>
          ) : (
            <span style={{ color: "var(--color-text-quaternary)" }}>Ready</span>
          )}
        </div>
        <div className="flex items-center gap-3">
          <span style={{ color: "var(--color-text-quaternary)" }}>
            {currentModel ? `Model: ${currentModel}` : "No model loaded"}
          </span>
          {(segments.length > 0 || (viewingSessionId && sessionExportSegments.length > 0)) && (
            <ExportMenu segments={viewingSessionId ? sessionExportSegments : segments} />
          )}
        </div>
      </footer>
    </div>
  );
}

export default App;
