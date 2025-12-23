import { useEffect, useCallback, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useRecordingStore } from "../stores/recording-store";
import {
  useActionRouterStore,
  type RouterStatusEvent,
  type WikipediaCityEvent,
  type WikipediaCityErrorEvent,
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

        // Atomically replace preview with final transcript
        // Use stable keys based on timestamps to prevent React re-renders
        const finalSegments = segments.map((seg) => ({
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

  const handleAudioChunk = useCallback(
    async (samples: number[]) => {
      try {
        const result = await invoke<StreamingResult | null>(
          "plugin:gibberish-stt|transcribe_streaming_chunk",
          { audioChunk: samples }
        );

        if (result) {
          setBufferDuration(result.buffer_duration_ms);
          if (result.text || result.volatile_text) {
            setPartialText(result.text);
            setVolatileText(result.volatile_text);
            setIsTranscribing(true);
          }
        }
      } catch (err) {
        // Silently ignore errors during streaming - don't spam console
      }
    },
    [setPartialText, setVolatileText, setBufferDuration, setIsTranscribing]
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
            // Reset streaming buffer
            try {
              await invoke("plugin:gibberish-stt|reset_streaming_buffer");
            } catch (err) {
              console.error("Failed to reset streaming buffer:", err);
            }
            // Transcribe the full file for final accurate results
            await transcribeFile(event.payload.path);
          }
        }
      );
      if (mounted) unlisteners.push(stopped);

      const error = await listen<string>("recorder:error", (event) => {
        if (mounted) console.error("Recording error:", event.payload);
      });
      if (mounted) unlisteners.push(error);

      // Listen for audio chunks for real-time transcription
      const audioChunk = await listen<number[]>(
        "recorder:audio-chunk",
        (event) => {
          if (mounted) {
            handleAudioChunk(event.payload);
          }
        }
      );
      if (mounted) unlisteners.push(audioChunk);

      const trayStart = await listen("tray:start-recording", () => {
        if (mounted) handleStartRecording();
      });
      if (mounted) unlisteners.push(trayStart);

      const trayStop = await listen("tray:stop-recording", () => {
        if (mounted) handleStopRecording();
      });
      if (mounted) unlisteners.push(trayStop);

      // Action router visibility (runs alongside streaming transcription)
      const routerStatus = await listen<RouterStatusEvent>(
        "tools:router_status",
        (event) => {
          if (!mounted) return;
          useActionRouterStore.getState().addEvent(event.payload);
          console.log("[router]", event.payload.phase, event.payload.payload);
        }
      );
      if (mounted) unlisteners.push(routerStatus);

      const wikiCity = await listen<WikipediaCityEvent>(
        "tools:wikipedia_city",
        (event) => {
          if (!mounted) return;
          useActionRouterStore.getState().setCityResult(event.payload);
          console.log("[router] wikipedia_city", event.payload.city);
        }
      );
      if (mounted) unlisteners.push(wikiCity);

      const wikiErr = await listen<WikipediaCityErrorEvent>(
        "tools:wikipedia_city_error",
        (event) => {
          if (!mounted) return;
          useActionRouterStore.getState().setCityError(event.payload);
          console.log("[router] wikipedia_city_error", event.payload.city, event.payload.error);
        }
      );
      if (mounted) unlisteners.push(wikiErr);
    };

    setupListeners();

    return () => {
      mounted = false;
      unlisteners.forEach((fn) => fn());
      listenersSetUp = false;
    };
  }, [transcribeFile, handleAudioChunk]);

  const handleStartRecording = async () => {
    try {
      clearSegments();
      setPartialText("");
      setVolatileText("");
      recordingStartTime.current = Date.now();
      await invoke("plugin:gibberish-stt|reset_streaming_buffer");
      await invoke("plugin:gibberish-recorder|start_recording", {
        sourceType: "combined_native",
      });
      startRecording();
    } catch (err) {
      console.error("Failed to start recording:", err);
    }
  };

  const handleStopRecording = async () => {
    try {
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
