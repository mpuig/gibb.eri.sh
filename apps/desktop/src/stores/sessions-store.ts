import { create } from "zustand";

export interface SessionSummary {
  id: string;
  title: string | null;
  createdAt: number;
  durationMs: number;
  preview: string;
}

export interface Session {
  id: string;
  title: string | null;
  createdAt: number;
  updatedAt: number;
  durationMs: number;
  segments: SessionSegment[];
}

export interface SessionSegment {
  id: string;
  text: string;
  startMs: number;
  endMs: number;
  speaker: number | null;
}

interface SessionsState {
  sessions: SessionSummary[];
  currentSession: Session | null;
  isLoading: boolean;
  searchQuery: string;

  setSessions: (sessions: SessionSummary[]) => void;
  setCurrentSession: (session: Session | null) => void;
  setIsLoading: (loading: boolean) => void;
  setSearchQuery: (query: string) => void;
  removeSession: (id: string) => void;
  updateSessionTitle: (id: string, title: string) => void;
}

export const useSessionsStore = create<SessionsState>((set) => ({
  sessions: [],
  currentSession: null,
  isLoading: false,
  searchQuery: "",

  setSessions: (sessions) => set({ sessions }),
  setCurrentSession: (session) => set({ currentSession: session }),
  setIsLoading: (loading) => set({ isLoading: loading }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  removeSession: (id) =>
    set((state) => ({
      sessions: state.sessions.filter((s) => s.id !== id),
      currentSession:
        state.currentSession?.id === id ? null : state.currentSession,
    })),
  updateSessionTitle: (id, title) =>
    set((state) => ({
      sessions: state.sessions.map((s) =>
        s.id === id ? { ...s, title } : s
      ),
      currentSession:
        state.currentSession?.id === id
          ? { ...state.currentSession, title }
          : state.currentSession,
    })),
}));
