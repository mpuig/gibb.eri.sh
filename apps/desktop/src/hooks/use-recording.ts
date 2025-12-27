import { useEffect, useCallback, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useRecordingStore } from "../stores/recording-store";
import { normalizeSherpaDisplayText } from "../lib/asr-text";
import {
  useActionRouterStore,
  type RouterStatusEvent,
  type SearchResultEvent,
  type SearchErrorEvent,
  type NoMatchEvent,
} from "../stores/action-router-store";
import { TranscriptSegment } from "./use-stt";
import { useSessions } from "./use-sessions";

// Global flag to prevent duplicate listener setup across hook instances
let listenersSetUp = false;

interface StreamingResult {
  text: string;
  volatile_text: string;
  is_partial: boolean;
  buffer_duration_ms: number;
}

interface StreamCommitEvent {
  text: string;
  ts_ms: number;
}

export function useRecording() {
  const {
    isRecording,
    startRecording,
    stopRecording,
    clearSegments,
    finalizeTranscript,
    setPartialText,
    setVolatileText,
    setBufferDuration,
    setIsTranscribing,
    setIsFinalizing,
    currentModel,
  } = useRecordingStore();
  const { saveSession } = useSessions();
  const [isTranscribing, setIsTranscribingLocal] = useState(false);
  const recordingStartTime = useRef<number>(0);

  const transcribeFile = useCallback(
    async (filePath: string) => {
      try {
        const currentModel = await invoke<string | null>(
          "plugin:gibberish-stt|get_current_model"
        );
        if (!currentModel) {
          console.log("No model loaded, skipping transcription");
          setIsFinalizing(false);
          return;
        }

        setIsTranscribingLocal(true);
        console.log("Transcribing file:", filePath);

        const segments = await invoke<TranscriptSegment[]>(
          "plugin:gibberish-stt|transcribe_file",
          { filePath }
        );

        const shouldNormalizeSherpa = currentModel.startsWith("sherpa-");
        const normalizedSegments = shouldNormalizeSherpa
          ? segments.map((seg) => ({
              ...seg,
              text: normalizeSherpaDisplayText(seg.text),
            }))
          : segments;

        // Atomically replace preview with final transcript
        // Use stable keys based on timestamps to prevent React re-renders
        const finalSegments = normalizedSegments.map((seg) => ({
          id: `seg-${seg.start_ms}-${seg.end_ms}`,
          text: seg.text,
          startMs: seg.start_ms,
          endMs: seg.end_ms,
          speaker: seg.speaker ?? undefined,
          isFinal: true,
        }));

        finalizeTranscript(finalSegments);
        console.log("Transcription complete:", segments.length, "segments");

        // Save session to database
        // Calculate duration from segments (more reliable than wall clock)
        const lastSegment = finalSegments[finalSegments.length - 1];
        const durationMs = lastSegment ? lastSegment.endMs : 0;
        if (finalSegments.length > 0 && durationMs > 0) {
          await saveSession(finalSegments, durationMs);
          console.log("Session saved to database");
        }
      } catch (err) {
        console.error("Transcription failed:", err);
        setIsFinalizing(false);
      } finally {
        setIsTranscribingLocal(false);
      }
    },
    [finalizeTranscript, setIsFinalizing, saveSession]
  );

  // Handle streaming results from Rust audio bus pipeline
  const handleStreamResult = useCallback(
    (result: StreamingResult) => {
      setBufferDuration(result.buffer_duration_ms);
      if (result.text || result.volatile_text) {
        const stableText = result.text?.trim() ?? "";
        const volatileText = result.volatile_text?.trim() ?? "";
        const shouldNormalizeSherpa = currentModel?.startsWith("sherpa-") ?? false;
        const mergedMain = stableText || volatileText;

        const nextPartialText = shouldNormalizeSherpa
          ? normalizeSherpaDisplayText(mergedMain)
          : mergedMain;
        const nextVolatileText = shouldNormalizeSherpa && stableText
          ? normalizeSherpaDisplayText(volatileText)
          : stableText
          ? volatileText
          : "";

        setPartialText(nextPartialText);
        setVolatileText(nextVolatileText);
        setIsTranscribing(true);
      }
    },
    [setPartialText, setVolatileText, setBufferDuration, setIsTranscribing, currentModel]
  );

  useEffect(() => {
    // Only set up listeners once globally to prevent duplicates
    if (listenersSetUp) return;
    listenersSetUp = true;

    let mounted = true;
    const unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      const started = await listen("recorder:started", async () => {
        if (mounted) {
          console.log("Recording started");
          // Reset streaming buffer when starting
          try {
            await invoke("plugin:gibberish-stt|reset_streaming_buffer");
          } catch (err) {
            console.error("Failed to reset streaming buffer:", err);
          }
        }
      });
      if (mounted) unlisteners.push(started);

      const stopped = await listen<{ path: string; duration_secs: number }>(
        "recorder:stopped",
        async (event) => {
          if (mounted) {
            console.log("Recording stopped:", event.payload);
            // Transcribe the full file for final accurate results
            await transcribeFile(event.payload.path);
            // Reset streaming buffer (and turn boundaries) after final transcription
            try {
              await invoke("plugin:gibberish-stt|reset_streaming_buffer");
            } catch (err) {
              console.error("Failed to reset streaming buffer:", err);
            }
          }
        }
      );
      if (mounted) unlisteners.push(stopped);

      const error = await listen<string>("recorder:error", (event) => {
        if (mounted) console.error("Recording error:", event.payload);
      });
      if (mounted) unlisteners.push(error);

      // Listen for streaming results from Rust audio bus pipeline
      const streamResult = await listen<StreamingResult>(
        "stt:stream_result",
        (event) => {
          if (mounted) {
            handleStreamResult(event.payload);
          }
        }
      );
      if (mounted) unlisteners.push(streamResult);

      // Listen for stream commits (for action router)
      const streamCommit = await listen<StreamCommitEvent>(
        "stt:stream_commit",
        (event) => {
          if (mounted) {
            console.log("[stt] commit:", event.payload.text);
          }
        }
      );
      if (mounted) unlisteners.push(streamCommit);

      const trayStart = await listen("tray:start-recording", () => {
        if (mounted) handleStartRecording();
      });
      if (mounted) unlisteners.push(trayStart);

      const trayStop = await listen("tray:stop-recording", () => {
        if (mounted) handleStopRecording();
      });
      if (mounted) unlisteners.push(trayStop);

      // Window visibility events for menu bar app behavior
      // Window shown = start listening for voice commands (action detection)
      const windowShown = await listen("tray:window-shown", async () => {
        if (!mounted) return;
        const { currentModel, isRecording } = useRecordingStore.getState();
        if (!currentModel) {
          console.log("[tray] Window shown - no model loaded, skipping listener start");
          return;
        }
        if (isRecording) {
          console.log("[tray] Window shown - already recording, skipping");
          return;
        }
        console.log("[tray] Window shown - starting audio capture for listening");
        try {
          // Reset streaming buffer
          await invoke("plugin:gibberish-stt|reset_streaming_buffer");
          // Start STT listener first
          await invoke("plugin:gibberish-stt|stt_start_listening");
          // Start audio capture (sends audio to the bus)
          await invoke("plugin:gibberish-recorder|start_recording", {
            sourceType: "combined_native",
          });
          useRecordingStore.getState().setIsListening(true);
        } catch (err) {
          console.error("Failed to start listening:", err);
        }
      });
      if (mounted) unlisteners.push(windowShown);

      // Window hidden = stop listening (privacy mode)
      const windowHidden = await listen("tray:window-hidden", async () => {
        if (!mounted) return;
        const { isRecording } = useRecordingStore.getState();
        console.log("[tray] Window hidden - stopping audio capture");
        useRecordingStore.getState().setIsListening(false);
        try {
          // Stop STT listener
          await invoke("plugin:gibberish-stt|stt_stop_listening");
          // Stop audio capture (discards the recording since we're just listening)
          if (!isRecording) {
            try {
              await invoke("plugin:gibberish-recorder|stop_recording");
            } catch {
              // Ignore errors if recorder wasn't running
            }
          }
        } catch (err) {
          console.error("Failed to stop listening:", err);
        }
      });
      if (mounted) unlisteners.push(windowHidden);

      // Action router visibility (runs alongside streaming transcription)
      const routerStatus = await listen<RouterStatusEvent>(
        "tools:router_status",
        (event) => {
          if (!mounted) return;
          useActionRouterStore.getState().addEvent(event.payload);
          console.log("[router]", event.payload.phase, event.payload.payload);

          // Handle no_match phase - show feedback to user
          if (event.payload.phase === "no_match") {
            const payload = event.payload.payload as NoMatchEvent;
            useActionRouterStore.getState().setNoMatch(payload);
          }
        }
      );
      if (mounted) unlisteners.push(routerStatus);

      const searchResult = await listen<SearchResultEvent>(
        "tools:search_result",
        (event) => {
          if (!mounted) return;
          useActionRouterStore.getState().setSearchResult(event.payload);
          console.log("[router] search_result", event.payload.query, event.payload.source);
        }
      );
      if (mounted) unlisteners.push(searchResult);

      const searchErr = await listen<SearchErrorEvent>(
        "tools:search_error",
        (event) => {
          if (!mounted) return;
          useActionRouterStore.getState().setSearchError(event.payload);
          console.log("[router] search_error", event.payload.query, event.payload.error);
        }
      );
      if (mounted) unlisteners.push(searchErr);
    };

    setupListeners();

    return () => {
      mounted = false;
      unlisteners.forEach((fn) => fn());
      listenersSetUp = false;
    };
  }, [transcribeFile, handleStreamResult]);

  const handleStartRecording = async () => {
    try {
      const { isListening } = useRecordingStore.getState();

      clearSegments();
      setPartialText("");
      setVolatileText("");
      recordingStartTime.current = Date.now();

      if (isListening) {
        // Already capturing audio in listen mode - promote to recording
        console.log("Promoting from listening to recording mode");
        await invoke("plugin:gibberish-recorder|promote_to_recording");
        useRecordingStore.getState().setIsListening(false);
      } else {
        // Fresh start - begin audio capture
        await invoke("plugin:gibberish-stt|reset_streaming_buffer");
        await invoke("plugin:gibberish-stt|stt_start_listening");
        await invoke("plugin:gibberish-recorder|start_recording", {
          sourceType: "combined_native",
        });
      }
      startRecording();
    } catch (err) {
      console.error("Failed to start recording:", err);
    }
  };

  const handleStopRecording = async () => {
    try {
      // Stop the audio bus listener
      await invoke("plugin:gibberish-stt|stt_stop_listening");
      const path = await invoke<string>("plugin:gibberish-recorder|stop_recording");
      stopRecording();
      console.log("Recording saved to:", path);
      return path;
    } catch (err) {
      console.error("Failed to stop recording:", err);
      return null;
    }
  };

  return {
    isRecording,
    isTranscribing,
    startRecording: handleStartRecording,
    stopRecording: handleStopRecording,
  };
}
