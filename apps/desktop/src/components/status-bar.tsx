import { useState } from "react";
import { useRecordingStore } from "../stores/recording-store";
import { useRecording } from "../hooks/use-recording";

interface PipelineMetrics {
  latencyMs?: number;
  rtf?: number;
}

export function StatusBar() {
  const { isRecording, isListening, currentModel } = useRecordingStore();
  const { isTranscribing, startRecording, stopRecording } = useRecording();
  const [showMetrics, setShowMetrics] = useState(false);

  // Pipeline metrics (placeholder - will be wired to real data later)
  const metrics: PipelineMetrics = {
    latencyMs: 250,
    rtf: 0.15,
  };

  const handleRecordClick = () => {
    if (isRecording) {
      stopRecording();
    } else {
      startRecording();
    }
  };

  const isDisabled = isTranscribing || (!isRecording && !currentModel);

  return (
    <footer
      className="px-4 py-2 flex items-center justify-between"
      style={{ borderTop: "1px solid var(--color-border)" }}
    >
      {/* Left: Listening Status with Metrics on Hover */}
      <div
        className="relative flex items-center gap-2"
        onMouseEnter={() => setShowMetrics(true)}
        onMouseLeave={() => setShowMetrics(false)}
      >
        {isListening ? (
          <div className="flex items-center gap-1.5 text-xs" style={{ color: "rgb(34, 197, 94)" }}>
            <span
              className="w-1.5 h-1.5 rounded-full animate-pulse"
              style={{ background: "rgb(34, 197, 94)" }}
            />
            Listening
          </div>
        ) : (
          <div className="flex items-center gap-1.5 text-xs" style={{ color: "var(--color-text-quaternary)" }}>
            <span className="w-1.5 h-1.5 rounded-full" style={{ background: "var(--color-text-quaternary)" }} />
            Not listening
          </div>
        )}

        {/* Metrics Tooltip */}
        {showMetrics && isListening && (
          <div
            className="absolute bottom-full left-0 mb-2 px-2 py-1.5 rounded-lg shadow-lg text-xs whitespace-nowrap"
            style={{
              background: "var(--color-bg-secondary)",
              border: "1px solid var(--color-border)",
            }}
          >
            <div className="flex gap-3">
              <div>
                <span style={{ color: "var(--color-text-tertiary)" }}>Latency: </span>
                <span style={{ color: "var(--color-text-primary)" }}>{metrics.latencyMs}ms</span>
              </div>
              <div>
                <span style={{ color: "var(--color-text-tertiary)" }}>RTF: </span>
                <span style={{ color: "var(--color-text-primary)" }}>{metrics.rtf?.toFixed(2)}</span>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Right: Record Toggle */}
      <button
        onClick={handleRecordClick}
        disabled={isDisabled}
        className="w-8 h-8 rounded-full flex items-center justify-center transition-all hover:scale-105"
        style={{
          cursor: isDisabled ? "not-allowed" : "pointer",
          opacity: isDisabled ? 0.5 : 1,
        }}
        title={
          isRecording
            ? "Stop Recording"
            : !currentModel
            ? "No Model Loaded"
            : "Start Recording"
        }
      >
        {isTranscribing ? (
          <svg
            className="animate-spin w-5 h-5"
            fill="none"
            viewBox="0 0 24 24"
            style={{ color: "var(--color-text-tertiary)" }}
          >
            <circle
              className="opacity-25"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              strokeWidth="4"
            />
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
            />
          </svg>
        ) : (
          <svg
            className="w-6 h-6"
            viewBox="0 0 24 24"
            fill={isRecording ? "#ef4444" : "var(--color-text-quaternary)"}
          >
            <circle cx="12" cy="12" r="8" />
          </svg>
        )}
      </button>
    </footer>
  );
}
