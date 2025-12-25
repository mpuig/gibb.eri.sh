import { useEffect, useState } from "react";
import { useSessions } from "../hooks/use-sessions";

interface SessionsSheetProps {
  isOpen: boolean;
  onClose: () => void;
}

function formatDate(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const days = Math.floor(diff / (1000 * 60 * 60 * 24));

  if (days === 0) {
    return `Today at ${date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  } else if (days === 1) {
    return `Yesterday`;
  } else if (days < 7) {
    return date.toLocaleDateString([], { weekday: "long" });
  } else {
    return date.toLocaleDateString([], { month: "short", day: "numeric" });
  }
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  }
  return `${seconds}s`;
}

export function SessionsSheet({ isOpen, onClose }: SessionsSheetProps) {
  const { sessions, isLoading, deleteSession, loadSession, currentSession, searchQuery, searchSessions } = useSessions();
  const [viewingSession, setViewingSession] = useState<string | null>(null);

  // Close on escape key
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (viewingSession) {
          setViewingSession(null);
        } else {
          onClose();
        }
      }
    };
    if (isOpen) {
      document.addEventListener("keydown", handleEscape);
    }
    return () => document.removeEventListener("keydown", handleEscape);
  }, [isOpen, onClose, viewingSession]);

  // Load session when selected
  useEffect(() => {
    if (viewingSession) {
      loadSession(viewingSession);
    }
  }, [viewingSession, loadSession]);

  if (!isOpen) return null;

  const handleCopySession = async () => {
    if (currentSession) {
      const text = currentSession.segments.map((s) => s.text).join("\n\n");
      await navigator.clipboard.writeText(text);
    }
  };

  return (
    <div className="fixed inset-0 z-50">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={() => {
          if (viewingSession) {
            setViewingSession(null);
          } else {
            onClose();
          }
        }}
      />

      {/* Sheet */}
      <div
        className="absolute bottom-0 left-0 right-0 rounded-t-2xl overflow-hidden"
        style={{
          background: "var(--color-bg-primary)",
          maxHeight: "85vh",
          animation: "slideUp 0.2s ease-out",
        }}
      >
        {/* Handle */}
        <div className="flex justify-center py-3">
          <div className="w-10 h-1 rounded-full" style={{ background: "var(--color-border)" }} />
        </div>

        {/* Header */}
        <div className="flex items-center justify-between px-6 pb-4">
          {viewingSession ? (
            <button
              onClick={() => setViewingSession(null)}
              className="flex items-center gap-1"
              style={{ color: "var(--color-accent)" }}
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
              </svg>
              Back
            </button>
          ) : (
            <h2 className="text-lg font-semibold" style={{ color: "var(--color-text-primary)" }}>
              History
            </h2>
          )}
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-full flex items-center justify-center"
            style={{ background: "var(--color-bg-tertiary)" }}
          >
            <svg className="w-4 h-4" style={{ color: "var(--color-text-tertiary)" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="overflow-auto" style={{ maxHeight: "calc(85vh - 100px)" }}>
          {viewingSession && currentSession ? (
            // Session Detail View
            <div className="px-6 pb-8">
              <div className="flex items-center justify-between mb-4">
                <div>
                  <h3 className="font-medium" style={{ color: "var(--color-text-primary)" }}>
                    {currentSession.title || formatDate(currentSession.createdAt)}
                  </h3>
                  <span className="text-xs" style={{ color: "var(--color-text-quaternary)" }}>
                    {formatDuration(currentSession.durationMs)}
                  </span>
                </div>
                <button
                  onClick={handleCopySession}
                  className="px-3 py-1.5 rounded-lg text-sm"
                  style={{ background: "var(--color-bg-tertiary)", color: "var(--color-text-secondary)" }}
                >
                  Copy
                </button>
              </div>
              <div className="space-y-3">
                {currentSession.segments.map((segment) => (
                  <div
                    key={segment.id}
                    className="p-3 rounded-xl"
                    style={{ background: "var(--color-bg-secondary)" }}
                  >
                    <p className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
                      {segment.text}
                    </p>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            // Sessions List
            <div className="pb-8">
              {/* Search Box */}
              <div className="px-6 pb-4">
                <div className="relative">
                  <svg
                    className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
                    style={{ color: "var(--color-text-quaternary)" }}
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                  <input
                    type="text"
                    placeholder="Search transcripts..."
                    value={searchQuery}
                    onChange={(e) => searchSessions(e.target.value)}
                    className="w-full pl-10 pr-4 py-2 rounded-xl text-sm"
                    style={{
                      background: "var(--color-bg-secondary)",
                      border: "1px solid var(--color-border)",
                      color: "var(--color-text-primary)",
                    }}
                  />
                </div>
              </div>

              {isLoading && sessions.length === 0 ? (
                <div className="py-12 text-center">
                  <svg className="animate-spin h-6 w-6 mx-auto mb-3" style={{ color: "var(--color-text-tertiary)" }} fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                  </svg>
                  <span style={{ color: "var(--color-text-tertiary)" }}>Loading...</span>
                </div>
              ) : sessions.length === 0 ? (
                <div className="py-12 text-center">
                  <svg
                    className="w-12 h-12 mx-auto mb-3"
                    style={{ color: "var(--color-text-quaternary)", opacity: 0.5 }}
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1}
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
                  </svg>
                  <p style={{ color: "var(--color-text-tertiary)" }}>No recordings yet</p>
                </div>
              ) : (
                sessions.map((session) => (
                  <div
                    key={session.id}
                    className="px-6 py-3 flex items-center justify-between cursor-pointer transition-colors"
                    style={{ borderBottom: "1px solid var(--color-border)" }}
                    onClick={() => setViewingSession(session.id)}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = "var(--color-bg-secondary)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = "transparent";
                    }}
                  >
                    <div className="flex-1 min-w-0">
                      <h3 className="font-medium text-sm truncate" style={{ color: "var(--color-text-primary)" }}>
                        {session.title || formatDate(session.createdAt)}
                      </h3>
                      <div className="flex items-center gap-2 mt-0.5">
                        <span className="text-xs" style={{ color: "var(--color-text-quaternary)" }}>
                          {formatDate(session.createdAt)}
                        </span>
                        <span style={{ color: "var(--color-text-quaternary)" }}>·</span>
                        <span className="text-xs" style={{ color: "var(--color-text-quaternary)" }}>
                          {formatDuration(session.durationMs)}
                        </span>
                      </div>
                      {session.preview && (
                        <p className="mt-1 text-xs truncate" style={{ color: "var(--color-text-tertiary)" }}>
                          {session.preview}
                        </p>
                      )}
                    </div>
                    <div className="flex items-center gap-2 ml-3">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          deleteSession(session.id);
                        }}
                        className="p-2 rounded-lg opacity-0 group-hover:opacity-100 transition-opacity"
                        style={{ color: "var(--color-text-quaternary)" }}
                        title="Delete"
                      >
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                      <svg className="w-4 h-4" style={{ color: "var(--color-text-quaternary)" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
                      </svg>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </div>

      <style>{`
        @keyframes slideUp {
          from { transform: translateY(100%); }
          to { transform: translateY(0); }
        }
      `}</style>
    </div>
  );
}
