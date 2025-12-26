import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export type Mode = "Meeting" | "Dev" | "Writer" | "Global";

export interface ContextState {
  mode: Mode;
  detectedMode: Mode;
  pinnedMode: Mode | null;
  activeApp: string | null;
  activeAppName: string | null;
  isMeeting: boolean;
  timestampMs: number;
}

interface ContextStore {
  context: ContextState;
  isLoading: boolean;
  setContext: (context: ContextState) => void;
  pinMode: (mode: Mode) => Promise<void>;
  unpinMode: () => Promise<void>;
  initialize: () => Promise<void>;
}

const DEFAULT_CONTEXT: ContextState = {
  mode: "Global",
  detectedMode: "Global",
  pinnedMode: null,
  activeApp: null,
  activeAppName: null,
  isMeeting: false,
  timestampMs: 0,
};

export const useContextStore = create<ContextStore>((set, get) => ({
  context: DEFAULT_CONTEXT,
  isLoading: true,

  setContext: (context) => set({ context }),

  pinMode: async (mode) => {
    try {
      const result = await invoke<ContextState>("plugin:gibberish-tools|pin_context_mode", {
        mode,
      });
      set({
        context: {
          mode: result.mode,
          detectedMode: result.detectedMode,
          pinnedMode: result.pinnedMode,
          activeApp: result.activeApp,
          activeAppName: result.activeAppName,
          isMeeting: result.isMeeting,
          timestampMs: result.timestampMs,
        },
      });
    } catch (e) {
      console.error("Failed to pin mode:", e);
    }
  },

  unpinMode: async () => {
    try {
      const result = await invoke<ContextState>("plugin:gibberish-tools|unpin_context_mode");
      set({
        context: {
          mode: result.mode,
          detectedMode: result.detectedMode,
          pinnedMode: result.pinnedMode,
          activeApp: result.activeApp,
          activeAppName: result.activeAppName,
          isMeeting: result.isMeeting,
          timestampMs: result.timestampMs,
        },
      });
    } catch (e) {
      console.error("Failed to unpin mode:", e);
    }
  },

  initialize: async () => {
    // Fetch initial context
    try {
      const result = await invoke<ContextState>("plugin:gibberish-tools|get_context");
      set({
        context: {
          mode: result.mode,
          detectedMode: result.detectedMode,
          pinnedMode: result.pinnedMode,
          activeApp: result.activeApp,
          activeAppName: result.activeAppName,
          isMeeting: result.isMeeting,
          timestampMs: result.timestampMs,
        },
        isLoading: false,
      });
    } catch (e) {
      console.error("Failed to get initial context:", e);
      set({ isLoading: false });
    }

    // Listen for context changes
    listen<{
      mode: Mode;
      active_app: string | null;
      active_app_name: string | null;
      is_meeting: boolean;
      timestamp_ms: number;
    }>("context:changed", (event) => {
      const { mode, active_app, active_app_name, is_meeting, timestamp_ms } = event.payload;
      const currentContext = get().context;

      set({
        context: {
          mode,
          detectedMode: mode,
          pinnedMode: currentContext.pinnedMode,
          activeApp: active_app,
          activeAppName: active_app_name,
          isMeeting: is_meeting,
          timestampMs: timestamp_ms,
        },
      });
    });
  },
}));
