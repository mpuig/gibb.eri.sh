import { useEffect } from "react";
import { useSessions } from "../hooks/use-sessions";

function formatTime(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${minutes}:${secs.toString().padStart(2, "0")}`;
}

function formatDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleDateString([], {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}h ${minutes % 60}m ${seconds % 60}s`;
  } else if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  } else {
    return `${seconds}s`;
  }
}

interface SessionViewerProps {
  sessionId: string;
  onBack: () => void;
}

export function SessionViewer({ sessionId, onBack }: SessionViewerProps) {
  const { currentSession, isLoading, loadSession, clearCurrentSession } =
    useSessions();

  useEffect(() => {
    loadSession(sessionId);
    return () => clearCurrentSession();
  }, [sessionId, loadSession, clearCurrentSession]);

  if (isLoading || !currentSession) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500">
        Loading session...
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="p-4 border-b border-gray-800">
        <div className="flex items-center gap-3">
          <button
            onClick={onBack}
            className="p-1 text-gray-400 hover:text-white rounded"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div>
            <h2 className="font-medium text-white">
              {currentSession.title || formatDate(currentSession.createdAt)}
            </h2>
            <p className="text-sm text-gray-500">
              {formatDuration(currentSession.durationMs)} - {currentSession.segments.length} segments
            </p>
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-4">
        <div className="space-y-3">
          {currentSession.segments.map((segment) => (
            <div key={segment.id} className="p-3 rounded-lg bg-gray-800">
              <div className="flex items-center gap-2 mb-1">
                <span className="text-xs text-gray-500">
                  {formatTime(segment.startMs)}
                </span>
                {segment.speaker !== null && (
                  <span className="text-xs px-2 py-0.5 rounded bg-blue-600/30 text-blue-400">
                    Speaker {segment.speaker + 1}
                  </span>
                )}
              </div>
              <p className="text-gray-200 leading-relaxed">{segment.text}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
