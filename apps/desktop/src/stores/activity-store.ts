import { create } from "zustand";
import type {
  Activity,
  ActivityFilter,
  ActivityStatus,
} from "../types/activity";

interface ActivityState {
  activities: Activity[];
  filter: ActivityFilter;
  searchQuery: string;

  // Mutations
  addActivity: (activity: Activity) => void;
  updateActivity: (id: string, updates: Partial<Activity>) => void;
  updateActivityStatus: (id: string, status: ActivityStatus) => void;
  removeActivity: (id: string) => void;
  clearActivities: () => void;
  setFilter: (filter: ActivityFilter) => void;
  setSearchQuery: (query: string) => void;
  toggleExpanded: (id: string) => void;

  // Queries (computed from state)
  getFilteredActivities: () => Activity[];
  getRecentTranscripts: (seconds: number) => Activity[];
  getChildActivities: (parentId: string) => Activity[];
  findActivityById: (id: string) => Activity | undefined;
}

const MAX_ACTIVITIES = 500;

export const useActivityStore = create<ActivityState>((set, get) => ({
  activities: [],
  filter: "all",
  searchQuery: "",

  addActivity: (activity) =>
    set((state) => {
      const activities = [activity, ...state.activities].slice(0, MAX_ACTIVITIES);
      return { activities };
    }),

  updateActivity: (id, updates) =>
    set((state) => ({
      activities: state.activities.map((a) =>
        a.id === id ? { ...a, ...updates } : a
      ),
    })),

  updateActivityStatus: (id, status) =>
    set((state) => ({
      activities: state.activities.map((a) =>
        a.id === id ? { ...a, status } : a
      ),
    })),

  removeActivity: (id) =>
    set((state) => ({
      activities: state.activities.filter((a) => a.id !== id),
    })),

  clearActivities: () => set({ activities: [] }),

  setFilter: (filter) => set({ filter }),

  setSearchQuery: (query) => set({ searchQuery: query }),

  toggleExpanded: (id) =>
    set((state) => ({
      activities: state.activities.map((a) =>
        a.id === id ? { ...a, expanded: !a.expanded } : a
      ),
    })),

  getFilteredActivities: () => {
    const { activities, filter, searchQuery } = get();
    const query = searchQuery.toLowerCase().trim();

    return activities.filter((a) => {
      // Filter by type (exclude context_change by default unless explicitly selected)
      if (filter === "all") {
        if (a.type === "context_change") return false;
      } else if (a.type !== filter) {
        return false;
      }

      // Filter by search query
      if (query) {
        const searchableText = [
          a.content.text,
          a.content.tool,
          a.content.error,
          a.content.mode,
          a.content.app,
        ]
          .filter(Boolean)
          .join(" ")
          .toLowerCase();
        return searchableText.includes(query);
      }

      return true;
    });
  },

  getRecentTranscripts: (seconds) => {
    const { activities } = get();
    const cutoff = Date.now() - seconds * 1000;
    return activities.filter(
      (a) => a.type === "transcript" && a.timestamp >= cutoff
    );
  },

  getChildActivities: (parentId) => {
    const { activities } = get();
    return activities.filter((a) => a.parentId === parentId);
  },

  findActivityById: (id) => {
    const { activities } = get();
    return activities.find((a) => a.id === id);
  },
}));

// Helper to generate unique IDs
export function generateActivityId(): string {
  return crypto.randomUUID();
}

// Factory functions for creating activities
export function createTranscriptActivity(text: string): Activity {
  return {
    id: generateActivityId(),
    type: "transcript",
    timestamp: Date.now(),
    status: "completed",
    content: { text },
  };
}

export function createVoiceCommandActivity(
  text: string,
  tool: string,
  args?: Record<string, unknown>
): Activity {
  return {
    id: generateActivityId(),
    type: "voice_command",
    timestamp: Date.now(),
    status: "running",
    content: { text, tool, args },
  };
}

export function createToolResultActivity(
  parentId: string,
  tool: string,
  result: Record<string, unknown>
): Activity {
  return {
    id: generateActivityId(),
    type: "tool_result",
    timestamp: Date.now(),
    status: "completed",
    parentId,
    content: { tool, result },
  };
}

export function createToolErrorActivity(
  parentId: string,
  tool: string,
  error: string
): Activity {
  return {
    id: generateActivityId(),
    type: "tool_error",
    timestamp: Date.now(),
    status: "error",
    parentId,
    content: { tool, error },
  };
}

export function createRecordingActivity(durationMs: number): Activity {
  return {
    id: generateActivityId(),
    type: "recording",
    timestamp: Date.now(),
    status: "completed",
    content: { duration: durationMs },
  };
}

export function createContextChangeActivity(
  prevMode: string,
  newMode: string,
  app?: string
): Activity {
  return {
    id: generateActivityId(),
    type: "context_change",
    timestamp: Date.now(),
    status: "completed",
    content: { prevMode, mode: newMode, app },
  };
}
