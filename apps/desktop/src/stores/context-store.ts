import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export type Mode = "Meeting" | "Dev" | "Writer" | "Global";

type WireMode = "meeting" | "dev" | "writer" | "global";

const toWireMode = (mode: Mode): WireMode => mode.toLowerCase() as WireMode;

const fromWireMode = (wire: WireMode): Mode => {
  const map: Record<WireMode, Mode> = {
    meeting: "Meeting",
    dev: "Dev",
    writer: "Writer",
    global: "Global",
  };
  return map[wire] ?? "Global";
};

interface WireContextState {
  mode: WireMode;
  detected_mode: WireMode;
  pinned_mode: WireMode | null;
  active_app: string | null;
  active_app_name: string | null;
  is_meeting: boolean;
  timestamp_ms: number;
}

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
      const result = await invoke<WireContextState>("plugin:gibberish-tools|pin_context_mode", {
        mode: toWireMode(mode),
      });
      set({
        context: {
          mode: fromWireMode(result.mode),
          detectedMode: fromWireMode(result.detected_mode),
          pinnedMode: result.pinned_mode ? fromWireMode(result.pinned_mode) : null,
          activeApp: result.active_app,
          activeAppName: result.active_app_name,
          isMeeting: result.is_meeting,
          timestampMs: result.timestamp_ms,
        },
      });
    } catch (e) {
      console.error("Failed to pin mode:", e);
    }
  },

  unpinMode: async () => {
    try {
      const result = await invoke<WireContextState>("plugin:gibberish-tools|unpin_context_mode");
      set({
        context: {
          mode: fromWireMode(result.mode),
          detectedMode: fromWireMode(result.detected_mode),
          pinnedMode: result.pinned_mode ? fromWireMode(result.pinned_mode) : null,
          activeApp: result.active_app,
          activeAppName: result.active_app_name,
          isMeeting: result.is_meeting,
          timestampMs: result.timestamp_ms,
        },
      });
    } catch (e) {
      console.error("Failed to unpin mode:", e);
    }
  },

  initialize: async () => {
    // Fetch initial context
    try {
      const result = await invoke<WireContextState>("plugin:gibberish-tools|get_context");
      set({
        context: {
          mode: fromWireMode(result.mode),
          detectedMode: fromWireMode(result.detected_mode),
          pinnedMode: result.pinned_mode ? fromWireMode(result.pinned_mode) : null,
          activeApp: result.active_app,
          activeAppName: result.active_app_name,
          isMeeting: result.is_meeting,
          timestampMs: result.timestamp_ms,
        },
        isLoading: false,
      });
    } catch (e) {
      console.error("Failed to get initial context:", e);
      set({ isLoading: false });
    }

    // Listen for context changes
    listen<{
      mode: WireMode;
      active_app: string | null;
      active_app_name: string | null;
      is_meeting: boolean;
      timestamp_ms: number;
    }>("context:changed", (event) => {
      const { mode, active_app, active_app_name, is_meeting, timestamp_ms } = event.payload;
      const currentContext = get().context;

      set({
        context: {
          mode: fromWireMode(mode),
          detectedMode: fromWireMode(mode),
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
