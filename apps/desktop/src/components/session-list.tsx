import { useState } from "react";
import { useSessions } from "../hooks/use-sessions";

function formatDate(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const days = Math.floor(diff / (1000 * 60 * 60 * 24));

  if (days === 0) {
    return `Today at ${date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  } else if (days === 1) {
    return `Yesterday at ${date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  } else if (days < 7) {
    return date.toLocaleDateString([], { weekday: "long", hour: "2-digit", minute: "2-digit" });
  } else {
    return date.toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" });
  }
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  } else {
    return `${seconds}s`;
  }
}

interface SessionListProps {
  onSelectSession: (id: string) => void;
}

export function SessionList({ onSelectSession }: SessionListProps) {
  const {
    sessions,
    isLoading,
    searchQuery,
    searchSessions,
    deleteSession,
    renameSession,
  } = useSessions();

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
    searchSessions(e.target.value);
  };

  const handleStartEdit = (id: string, currentTitle: string | null) => {
    setEditingId(id);
    setEditTitle(currentTitle || "");
  };

  const handleSaveEdit = async (id: string) => {
    if (editTitle.trim()) {
      await renameSession(id, editTitle.trim());
    }
    setEditingId(null);
  };

  const handleDelete = async (id: string) => {
    await deleteSession(id);
    setConfirmDeleteId(null);
  };

  if (isLoading && sessions.length === 0) {
    return (
      <div className="empty-state h-full">
        <svg className="animate-spin h-6 w-6 mb-3" style={{ color: "var(--color-text-tertiary)" }} fill="none" viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
        </svg>
        <span style={{ color: "var(--color-text-tertiary)" }}>Loading sessions...</span>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Search */}
      <div className="p-4 border-b" style={{ borderColor: "var(--color-border)" }}>
        <div className="relative">
          <svg
            className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
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
            placeholder="Search transcripts..."
            value={searchQuery}
            onChange={handleSearch}
            className="input pl-10"
          />
        </div>
      </div>

      {/* Session List */}
      <div className="flex-1 overflow-auto">
        {sessions.length === 0 ? (
          <div className="empty-state h-full">
            <svg
              className="w-16 h-16 mb-4"
              style={{ color: "var(--color-text-quaternary)", opacity: 0.3 }}
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={1}
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
            <h3 className="text-base font-medium mb-1" style={{ color: "var(--color-text-primary)" }}>
              {searchQuery ? "No Results" : "No Sessions Yet"}
            </h3>
            <p style={{ color: "var(--color-text-tertiary)" }}>
              {searchQuery
                ? "Try a different search term"
                : "Start recording to create your first transcript"}
            </p>
          </div>
        ) : (
          <div>
            {sessions.map((session) => (
              <div
                key={session.id}
                className="px-4 py-3 cursor-pointer transition-colors duration-150 group"
                style={{ borderBottom: "1px solid var(--color-border-subtle)" }}
                onClick={() => onSelectSession(session.id)}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = "var(--color-bg-secondary)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "transparent";
                }}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1 min-w-0">
                    {editingId === session.id ? (
                      <input
                        type="text"
                        value={editTitle}
                        onChange={(e) => setEditTitle(e.target.value)}
                        onBlur={() => handleSaveEdit(session.id)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") handleSaveEdit(session.id);
                          if (e.key === "Escape") setEditingId(null);
                        }}
                        onClick={(e) => e.stopPropagation()}
                        autoFocus
                        className="input py-1 text-sm"
                      />
                    ) : (
                      <h3
                        className="font-medium truncate text-sm"
                        style={{ color: "var(--color-text-primary)" }}
                      >
                        {session.title || formatDate(session.createdAt)}
                      </h3>
                    )}
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
                      <p
                        className="mt-1.5 text-sm line-clamp-2"
                        style={{ color: "var(--color-text-tertiary)" }}
                      >
                        {session.preview}
                      </p>
                    )}
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleStartEdit(session.id, session.title);
                      }}
                      className="p-1.5 rounded-md transition-colors"
                      style={{ color: "var(--color-text-tertiary)" }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.background = "var(--color-bg-tertiary)";
                        e.currentTarget.style.color = "var(--color-text-primary)";
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.background = "transparent";
                        e.currentTarget.style.color = "var(--color-text-tertiary)";
                      }}
                      title="Rename"
                    >
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                      </svg>
                    </button>
                    {confirmDeleteId === session.id ? (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDelete(session.id);
                        }}
                        className="p-1.5 rounded-md"
                        style={{ color: "var(--color-danger)", background: "rgba(255, 69, 58, 0.1)" }}
                        title="Confirm delete"
                      >
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                        </svg>
                      </button>
                    ) : (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setConfirmDeleteId(session.id);
                        }}
                        className="p-1.5 rounded-md transition-colors"
                        style={{ color: "var(--color-text-tertiary)" }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.background = "rgba(255, 69, 58, 0.1)";
                          e.currentTarget.style.color = "var(--color-danger)";
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.background = "transparent";
                          e.currentTarget.style.color = "var(--color-text-tertiary)";
                        }}
                        title="Delete"
                      >
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
