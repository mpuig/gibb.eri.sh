import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  useActivityStore,
  createTranscriptActivity,
  createVoiceCommandActivity,
  createToolResultActivity,
  createToolErrorActivity,
  createRecordingActivity,
  createContextChangeActivity,
} from "../stores/activity-store";
import { useContextStore } from "../stores/context-store";

interface StreamCommitEvent {
  text: string;
  ts_ms: number;
}

interface RouterStatusPayload {
  phase: string;
  payload: {
    tool?: string;
    args?: Record<string, unknown>;
    result?: Record<string, unknown>;
    error?: string;
    text?: string;
    evidence?: string;
  };
}

interface ContextChangedEvent {
  mode: string;
  active_app: string | null;
  active_app_name: string | null;
  is_meeting: boolean;
  timestamp_ms: number;
}

// Track active voice command for linking tool results
let activeVoiceCommandId: string | null = null;

export function useActivityEvents() {
  const addActivity = useActivityStore((s) => s.addActivity);
  const updateActivityStatus = useActivityStore((s) => s.updateActivityStatus);
  const lastModeRef = useRef<string | null>(null);
  const recordingStartRef = useRef<number | null>(null);

  useEffect(() => {
    let mounted = true;
    const unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      // Listen for stream commits (transcripts)
      const streamCommit = await listen<StreamCommitEvent>(
        "stt:stream_commit",
        (event) => {
          if (!mounted) return;
          const text = event.payload.text?.trim();
          if (text) {
            const activity = createTranscriptActivity(text);
            addActivity(activity);
          }
        }
      );
      if (mounted) unlisteners.push(streamCommit);

      // Listen for router status (voice commands and tool results)
      const routerStatus = await listen<RouterStatusPayload>(
        "tools:router_status",
        (event) => {
          if (!mounted) return;
          const { phase, payload } = event.payload;

          switch (phase) {
            case "tool_start": {
              // Create voice command activity
              const text = payload.evidence || payload.text || "";
              const tool = payload.tool || "unknown";
              const activity = createVoiceCommandActivity(text, tool, payload.args);
              activeVoiceCommandId = activity.id;
              addActivity(activity);
              break;
            }
            case "tool_result": {
              // Create tool result activity linked to voice command
              if (activeVoiceCommandId) {
                const tool = payload.tool || "unknown";
                const result = payload.result || {};
                const activity = createToolResultActivity(
                  activeVoiceCommandId,
                  tool,
                  result
                );
                addActivity(activity);
                // Mark parent voice command as completed
                updateActivityStatus(activeVoiceCommandId, "completed");
                activeVoiceCommandId = null;
              }
              break;
            }
            case "tool_error": {
              // Create tool error activity linked to voice command
              if (activeVoiceCommandId) {
                const tool = payload.tool || "unknown";
                const error = payload.error || "Unknown error";
                const activity = createToolErrorActivity(
                  activeVoiceCommandId,
                  tool,
                  error
                );
                addActivity(activity);
                // Mark parent voice command as error
                updateActivityStatus(activeVoiceCommandId, "error");
                activeVoiceCommandId = null;
              }
              break;
            }
            case "no_match": {
              // Clear active voice command on no match
              if (activeVoiceCommandId) {
                updateActivityStatus(activeVoiceCommandId, "completed");
                activeVoiceCommandId = null;
              }
              break;
            }
          }
        }
      );
      if (mounted) unlisteners.push(routerStatus);

      // Listen for tool errors
      const toolError = await listen<{ tool: string; error: string }>(
        "tools:tool_error",
        (event) => {
          if (!mounted) return;
          const { tool, error } = event.payload;
          if (activeVoiceCommandId) {
            const activity = createToolErrorActivity(
              activeVoiceCommandId,
              tool,
              error
            );
            addActivity(activity);
            updateActivityStatus(activeVoiceCommandId, "error");
            activeVoiceCommandId = null;
          }
        }
      );
      if (mounted) unlisteners.push(toolError);

      // Listen for context changes (only Mode Transitions)
      const contextChanged = await listen<ContextChangedEvent>(
        "context:changed",
        (event) => {
          if (!mounted) return;
          const newMode = event.payload.mode;
          const prevMode = lastModeRef.current;

          // Only persist Mode Transitions (not window focus changes)
          if (prevMode && prevMode !== newMode) {
            const activity = createContextChangeActivity(
              prevMode,
              newMode,
              event.payload.active_app_name || undefined
            );
            addActivity(activity);
          }

          lastModeRef.current = newMode;
        }
      );
      if (mounted) unlisteners.push(contextChanged);

      // Listen for recording start
      const recorderStarted = await listen("recorder:started", () => {
        if (!mounted) return;
        recordingStartRef.current = Date.now();
      });
      if (mounted) unlisteners.push(recorderStarted);

      // Listen for recording stop
      const recorderStopped = await listen<{ path: string; duration_secs: number }>(
        "recorder:stopped",
        (event) => {
          if (!mounted) return;
          const durationMs = event.payload.duration_secs * 1000;
          if (durationMs > 0) {
            const activity = createRecordingActivity(durationMs);
            addActivity(activity);
          }
          recordingStartRef.current = null;
        }
      );
      if (mounted) unlisteners.push(recorderStopped);
    };

    // Initialize lastModeRef with current context
    const currentMode = useContextStore.getState().context.mode;
    lastModeRef.current = currentMode;

    setupListeners();

    return () => {
      mounted = false;
      unlisteners.forEach((fn) => fn());
    };
  }, [addActivity, updateActivityStatus]);
}
