import { useRecording } from "../hooks/use-recording";
import { useRecordingStore } from "../stores/recording-store";

interface RecordingControlsProps {
  onRecordingStart?: () => void;
}

export function RecordingControls({ onRecordingStart }: RecordingControlsProps) {
  const { isRecording, isTranscribing, startRecording, stopRecording } = useRecording();
  const currentModel = useRecordingStore((state) => state.currentModel);

  const isDisabled = isTranscribing || (!isRecording && !currentModel);

  const handleClick = () => {
    if (isRecording) {
      stopRecording();
    } else {
      startRecording();
      onRecordingStart?.();
    }
  };

  // Stop button (when recording)
  if (isRecording) {
    return (
      <button
        onClick={handleClick}
        className="w-12 h-12 rounded-full flex items-center justify-center transition-all duration-200 hover:scale-105"
        style={{ background: "var(--color-danger)" }}
        title="Stop Recording"
      >
        {/* Stop icon: square (smaller) */}
        <div className="w-3 h-3 rounded-sm" style={{ background: "white" }} />
      </button>
    );
  }

  // Processing spinner
  if (isTranscribing) {
    return (
      <button
        disabled
        className="w-12 h-12 rounded-full flex items-center justify-center"
        style={{ background: "var(--color-bg-tertiary)", cursor: "not-allowed" }}
        title="Processing..."
      >
        <svg className="animate-spin w-5 h-5" fill="none" viewBox="0 0 24 24" style={{ color: "var(--color-text-tertiary)" }}>
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
        </svg>
      </button>
    );
  }

  // Record button (red circle with microphone icon)
  return (
    <button
      onClick={handleClick}
      disabled={isDisabled}
      className="w-12 h-12 rounded-full flex items-center justify-center transition-all duration-200 hover:scale-105"
      style={{
        background: isDisabled ? "var(--color-bg-tertiary)" : "var(--color-danger)",
        cursor: isDisabled ? "not-allowed" : "pointer",
        opacity: isDisabled ? 0.5 : 1,
      }}
      title={!currentModel ? "No Model Loaded" : "Start Recording"}
    >
      {/* Microphone icon */}
      <svg className="w-5 h-5" viewBox="0 0 24 24" fill={isDisabled ? "var(--color-text-quaternary)" : "white"}>
        <path d="M12 14a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v5a3 3 0 0 0 3 3z" />
        <path d="M19 11a1 1 0 1 0-2 0 5 5 0 0 1-10 0 1 1 0 1 0-2 0 7 7 0 0 0 6 6.92V20H8a1 1 0 1 0 0 2h8a1 1 0 1 0 0-2h-3v-2.08A7 7 0 0 0 19 11z" />
      </svg>
    </button>
  );
}
