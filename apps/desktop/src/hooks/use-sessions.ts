import { useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  useSessionsStore,
  type SessionSummary,
  type Session,
} from "../stores/sessions-store";

interface SessionSummaryDto {
  id: string;
  title: string | null;
  created_at: number;
  duration_ms: number;
  preview: string;
}

interface SessionDto {
  id: string;
  title: string | null;
  created_at: number;
  updated_at: number;
  duration_ms: number;
  segments: {
    id: string;
    text: string;
    start_ms: number;
    end_ms: number;
    speaker: number | null;
  }[];
}

function mapSessionSummary(dto: SessionSummaryDto): SessionSummary {
  return {
    id: dto.id,
    title: dto.title,
    createdAt: dto.created_at,
    durationMs: dto.duration_ms,
    preview: dto.preview,
  };
}

function mapSession(dto: SessionDto): Session {
  return {
    id: dto.id,
    title: dto.title,
    createdAt: dto.created_at,
    updatedAt: dto.updated_at,
    durationMs: dto.duration_ms,
    segments: dto.segments.map((s) => ({
      id: s.id,
      text: s.text,
      startMs: s.start_ms,
      endMs: s.end_ms,
      speaker: s.speaker,
    })),
  };
}

export function useSessions() {
  const {
    sessions,
    currentSession,
    isLoading,
    searchQuery,
    setSessions,
    setCurrentSession,
    setIsLoading,
    setSearchQuery,
    removeSession,
    updateSessionTitle,
  } = useSessionsStore();

  const loadSessions = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await invoke<SessionSummaryDto[]>(
        "plugin:gibberish-stt|list_sessions"
      );
      setSessions(result.map(mapSessionSummary));
    } catch (err) {
      console.error("Failed to load sessions:", err);
    } finally {
      setIsLoading(false);
    }
  }, [setSessions, setIsLoading]);

  const searchSessions = useCallback(
    async (query: string) => {
      setSearchQuery(query);
      setIsLoading(true);
      try {
        if (query.trim()) {
          const result = await invoke<SessionSummaryDto[]>(
            "plugin:gibberish-stt|search_sessions",
            { query }
          );
          setSessions(result.map(mapSessionSummary));
        } else {
          await loadSessions();
        }
      } catch (err) {
        console.error("Failed to search sessions:", err);
      } finally {
        setIsLoading(false);
      }
    },
    [setSessions, setIsLoading, setSearchQuery, loadSessions]
  );

  const loadSession = useCallback(
    async (id: string) => {
      setIsLoading(true);
      try {
        const result = await invoke<SessionDto>(
          "plugin:gibberish-stt|get_session",
          { id }
        );
        setCurrentSession(mapSession(result));
      } catch (err) {
        console.error("Failed to load session:", err);
      } finally {
        setIsLoading(false);
      }
    },
    [setCurrentSession, setIsLoading]
  );

  const deleteSession = useCallback(
    async (id: string) => {
      try {
        await invoke("plugin:gibberish-stt|delete_session", { id });
        removeSession(id);
      } catch (err) {
        console.error("Failed to delete session:", err);
      }
    },
    [removeSession]
  );

  const renameSession = useCallback(
    async (id: string, title: string) => {
      try {
        await invoke("plugin:gibberish-stt|update_session_title", { id, title });
        updateSessionTitle(id, title);
      } catch (err) {
        console.error("Failed to rename session:", err);
      }
    },
    [updateSessionTitle]
  );

  const saveSession = useCallback(
    async (
      segments: { id: string; text: string; startMs: number; endMs: number; speaker?: number }[],
      durationMs: number,
      title?: string
    ) => {
      try {
        const id = await invoke<string>("plugin:gibberish-stt|save_session", {
          segments: segments.map((s) => ({
            id: s.id,
            text: s.text,
            start_ms: s.startMs,
            end_ms: s.endMs,
            speaker: s.speaker ?? null,
          })),
          durationMs,
          title: title ?? null,
        });
        await loadSessions();
        return id;
      } catch (err) {
        console.error("Failed to save session:", err);
        return null;
      }
    },
    [loadSessions]
  );

  const clearCurrentSession = useCallback(() => {
    setCurrentSession(null);
  }, [setCurrentSession]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  return {
    sessions,
    currentSession,
    isLoading,
    searchQuery,
    loadSessions,
    searchSessions,
    loadSession,
    deleteSession,
    renameSession,
    saveSession,
    clearCurrentSession,
  };
}
