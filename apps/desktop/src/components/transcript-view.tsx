import { memo, useRef, useState, useEffect } from "react";
import { useRecordingStore, type TranscriptSegment } from "../stores/recording-store";

interface TranscriptViewProps {
  segments: TranscriptSegment[];
}

function formatTime(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${minutes}:${secs.toString().padStart(2, "0")}`;
}

const SegmentItem = memo(function SegmentItem({ segment }: { segment: TranscriptSegment }) {
  return (
    <div
      className="segment animate-in"
      style={{
        background: segment.isFinal ? "var(--color-bg-secondary)" : "rgba(44, 44, 46, 0.5)",
      }}
    >
      <div className="flex items-center gap-2 mb-2">
        <span
          className="text-xs font-medium tabular-nums"
          style={{ color: "var(--color-text-quaternary)" }}
        >
          {formatTime(segment.startMs)}
        </span>
        {segment.speaker !== undefined && (
          <span className="badge badge-speaker">
            Speaker {segment.speaker + 1}
          </span>
        )}
        {!segment.isFinal && (
          <span className="badge" style={{ background: "rgba(255, 214, 10, 0.2)", color: "var(--color-warning)" }}>
            preview
          </span>
        )}
      </div>
      <p
        className="leading-relaxed"
        style={{ color: "var(--color-text-secondary)", fontSize: "0.9375rem" }}
      >
        {segment.text}
      </p>
    </div>
  );
});

export const TranscriptView = memo(function TranscriptView({ segments }: TranscriptViewProps) {
  const { isRecording, isFinalizing, partialText, volatileText, bufferDurationMs } = useRecordingStore();
  const containerRef = useRef<HTMLDivElement>(null);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const lastContentLength = useRef(0);

  // Track scroll position to detect if user scrolled up
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const onScroll = () => {
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
      setIsAtBottom(atBottom);
    };

    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  // Auto-scroll only when at bottom and content changes
  useEffect(() => {
    const el = containerRef.current;
    if (!el || !isAtBottom) return;

    const currentLength = segments.length + (partialText?.length || 0) + (volatileText?.length || 0);
    if (currentLength !== lastContentLength.current) {
      lastContentLength.current = currentLength;
      requestAnimationFrame(() => {
        el.scrollTop = el.scrollHeight;
      });
    }
  }, [segments, partialText, isAtBottom]);

  const hasContent = segments.length > 0 || partialText || volatileText;

  if (!hasContent) {
    return (
      <div className="empty-state py-12">
        <div className="flex items-center gap-2" style={{ color: "var(--color-text-tertiary)" }}>
          {isRecording ? (
            <>
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75" style={{ background: "var(--color-text-tertiary)" }} />
                <span className="relative inline-flex rounded-full h-2 w-2" style={{ background: "var(--color-text-tertiary)" }} />
              </span>
              Listening for speech...
            </>
          ) : (
            "Start recording to transcribe"
          )}
        </div>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="space-y-3 max-w-3xl mx-auto overflow-auto">
      {segments.map((segment) => (
        <SegmentItem key={segment.id} segment={segment} />
      ))}

      {/* Live transcription */}
      {isRecording && (partialText || volatileText) && (
        <div className="segment segment-live animate-in">
          <div className="flex items-center gap-2 mb-2">
            <span
              className="text-xs font-medium tabular-nums"
              style={{ color: "var(--color-accent)" }}
            >
              {formatTime(bufferDurationMs)}
            </span>
            <span className="badge badge-live flex items-center gap-1.5">
              <span className="relative flex h-1.5 w-1.5">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75" style={{ background: "var(--color-accent)" }} />
                <span className="relative inline-flex rounded-full h-1.5 w-1.5" style={{ background: "var(--color-accent)" }} />
              </span>
              live
            </span>
          </div>
          <p
            className="leading-relaxed"
            style={{ color: "var(--color-text-secondary)", fontSize: "0.9375rem" }}
          >
            {partialText}
            {volatileText && (
              <span style={{ opacity: 0.5 }}>
                {partialText ? " " : ""}{volatileText}
              </span>
            )}
          </p>
        </div>
      )}

      {/* Finalizing indicator */}
      {!isRecording && isFinalizing && partialText && (
        <div className="segment segment-finalizing animate-in">
          <div className="flex items-center gap-2 mb-2">
            <span className="badge flex items-center gap-1.5" style={{ background: "rgba(255, 214, 10, 0.2)", color: "var(--color-warning)" }}>
              <svg className="animate-spin h-3 w-3" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
              finalizing
            </span>
          </div>
          <p
            className="leading-relaxed"
            style={{ color: "var(--color-text-secondary)", fontSize: "0.9375rem" }}
          >
            {partialText}
          </p>
        </div>
      )}

      {/* Listening indicator */}
      {isRecording && !partialText && !volatileText && segments.length === 0 && (
        <div className="empty-state py-8">
          <div className="flex items-center gap-2" style={{ color: "var(--color-text-tertiary)" }}>
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75" style={{ background: "var(--color-text-tertiary)" }} />
              <span className="relative inline-flex rounded-full h-2 w-2" style={{ background: "var(--color-text-tertiary)" }} />
            </span>
            Listening for speech...
          </div>
        </div>
      )}
    </div>
  );
})
