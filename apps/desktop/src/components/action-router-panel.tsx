import { memo } from "react";
import { useActionRouterStore } from "../stores/action-router-store";

function formatTs(tsMs: number): string {
  const d = new Date(tsMs);
  return d.toLocaleTimeString(undefined, { hour12: false }) + "." + String(d.getMilliseconds()).padStart(3, "0");
}

function previewPayload(payload: unknown): string {
  try {
    const s = JSON.stringify(payload);
    if (s.length <= 160) return s;
    return s.slice(0, 160) + "…";
  } catch {
    return String(payload);
  }
}

export const ActionRouterPanel = memo(function ActionRouterPanel() {
  const { events, lastSearchResult, lastSearchError, lastNoMatch, lastSummary, clear } = useActionRouterStore();

  return (
    <div className="max-w-3xl mx-auto">
      <div
        className="glass rounded-xl p-4"
        style={{ border: "1px solid var(--color-border)" }}
      >
        <div className="flex items-center justify-between mb-3">
          <div className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
            Actions
          </div>
          <button
            onClick={clear}
            className="text-xs px-2 py-1 rounded"
            style={{
              background: "var(--color-bg-secondary)",
              color: "var(--color-text-tertiary)",
              border: "1px solid var(--color-border)",
            }}
          >
            Clear
          </button>
        </div>

        {lastSearchResult && (
          <div
            className="rounded-lg p-3 mb-3"
            style={{ background: "var(--color-bg-secondary)" }}
          >
            <div className="text-xs mb-1" style={{ color: "var(--color-text-quaternary)" }}>
              {lastSearchResult.source}: {lastSearchResult.query}
            </div>
            <div className="text-sm font-medium mb-1" style={{ color: "var(--color-text-primary)" }}>
              {lastSearchResult.result.title}
            </div>
            <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              {lastSearchResult.result.summary}
            </div>
            <a
              href={lastSearchResult.result.url}
              className="text-xs mt-2 inline-block"
              style={{ color: "var(--color-accent)" }}
            >
              {lastSearchResult.result.url}
            </a>
          </div>
        )}

        {lastSearchError && (
          <div
            className="rounded-lg p-3 mb-3"
            style={{ background: "rgba(255, 69, 58, 0.10)", border: "1px solid rgba(255, 69, 58, 0.25)" }}
          >
            <div className="text-xs mb-1" style={{ color: "var(--color-text-quaternary)" }}>
              {lastSearchError.source} error: {lastSearchError.query}
            </div>
            <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              {lastSearchError.error}
            </div>
          </div>
        )}

        {lastNoMatch && (
          <div
            className="rounded-lg p-3 mb-3"
            style={{ background: "rgba(255, 159, 10, 0.10)", border: "1px solid rgba(255, 159, 10, 0.25)" }}
          >
            <div className="text-xs mb-1" style={{ color: "var(--color-text-quaternary)" }}>
              No matching action
            </div>
            <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              {lastNoMatch.message}
            </div>
            {lastNoMatch.text && (
              <div className="text-xs mt-1" style={{ color: "var(--color-text-quaternary)" }}>
                "{lastNoMatch.text}"
              </div>
            )}
          </div>
        )}

        {lastSummary && (
          <div
            className="rounded-lg p-3 mb-3"
            style={{ background: "rgba(99, 102, 241, 0.10)", border: "1px solid rgba(99, 102, 241, 0.25)" }}
          >
            <div className="text-sm" style={{ color: "var(--color-text-primary)" }}>
              {lastSummary.summary}
            </div>
          </div>
        )}

        {events.length === 0 ? (
          <div className="text-sm" style={{ color: "var(--color-text-tertiary)" }}>
            No action-router activity yet.
          </div>
        ) : (
          <div className="space-y-1 max-h-40 overflow-auto">
            {events.slice(-12).map((e, idx) => (
              <div
                key={`${e.ts_ms}-${idx}`}
                className="text-xs flex gap-2"
                style={{ color: "var(--color-text-tertiary)" }}
              >
                <span className="tabular-nums" style={{ color: "var(--color-text-quaternary)" }}>
                  {formatTs(e.ts_ms)}
                </span>
                <span style={{ color: "var(--color-text-secondary)" }}>{e.phase}</span>
                <span className="truncate">{previewPayload(e.payload)}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
});

