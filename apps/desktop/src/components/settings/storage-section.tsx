import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSessions } from "../../hooks/use-sessions";
import { SectionHeader } from "./shared";

export function StorageSection() {
  const { sessions, loadSessions } = useSessions();
  const [isClearing, setIsClearing] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);

  const handleClearAllSessions = async () => {
    setIsClearing(true);
    try {
      for (const session of sessions) {
        await invoke("plugin:gibberish-stt|delete_session", { id: session.id });
      }
      await loadSessions();
    } catch (err) {
      console.error("Failed to clear sessions:", err);
    } finally {
      setIsClearing(false);
      setShowConfirm(false);
    }
  };

  return (
    <section>
      <SectionHeader>Storage</SectionHeader>
      <div className="space-y-3">
        <div
          className="card p-4 flex items-center justify-between"
          style={{ background: "var(--color-bg-secondary)" }}
        >
          <div>
            <div className="font-medium text-sm" style={{ color: "var(--color-text-primary)" }}>
              Saved Sessions
            </div>
            <div className="text-xs mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
              {sessions.length} recording{sessions.length !== 1 ? "s" : ""} stored locally
            </div>
          </div>
          {showConfirm ? (
            <div className="flex items-center gap-2">
              <span className="text-xs" style={{ color: "var(--color-text-tertiary)" }}>
                Delete all?
              </span>
              <button
                onClick={handleClearAllSessions}
                disabled={isClearing}
                className="btn-danger text-xs px-2 py-1"
              >
                {isClearing ? "..." : "Yes"}
              </button>
              <button
                onClick={() => setShowConfirm(false)}
                className="btn-secondary text-xs px-2 py-1"
              >
                No
              </button>
            </div>
          ) : (
            <button
              onClick={() => setShowConfirm(true)}
              disabled={sessions.length === 0}
              className="btn-secondary text-sm"
              style={{ opacity: sessions.length === 0 ? 0.5 : 1 }}
            >
              Clear All
            </button>
          )}
        </div>
      </div>
    </section>
  );
}
