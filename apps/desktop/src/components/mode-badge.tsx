import { useEffect, useState, ReactNode } from "react";
import { useContextStore, Mode } from "../stores/context-store";

const MODE_CONFIG: Record<
  Mode,
  { label: string; color: string; icon: ReactNode }
> = {
  Meeting: {
    label: "Meeting",
    color: "#ef4444", // red
    icon: (
      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
      </svg>
    ),
  },
  Dev: {
    label: "Dev",
    color: "#3b82f6", // blue
    icon: (
      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
      </svg>
    ),
  },
  Writer: {
    label: "Writer",
    color: "#8b5cf6", // purple
    icon: (
      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
      </svg>
    ),
  },
  Global: {
    label: "Global",
    color: "#6b7280", // gray
    icon: (
      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    ),
  },
};

export function ModeBadge() {
  const { context, isLoading, initialize, pinMode, unpinMode } = useContextStore();
  const [showMenu, setShowMenu] = useState(false);
  const [prevMode, setPrevMode] = useState<Mode | null>(null);
  const [showToast, setShowToast] = useState(false);

  useEffect(() => {
    initialize();
  }, [initialize]);

  // Show toast on mode change
  useEffect(() => {
    if (prevMode !== null && prevMode !== context.mode) {
      setShowToast(true);
      const timer = setTimeout(() => setShowToast(false), 2000);
      return () => clearTimeout(timer);
    }
    setPrevMode(context.mode);
  }, [context.mode, prevMode]);

  if (isLoading) {
    return null;
  }

  // Fallback to Global if mode is invalid
  const mode = context.mode && MODE_CONFIG[context.mode] ? context.mode : "Global";
  const config = MODE_CONFIG[mode];
  const isPinned = context.pinnedMode !== null;

  return (
    <>
      {/* Mode Badge */}
      <div className="relative">
        <button
          onClick={() => setShowMenu(!showMenu)}
          className="flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium transition-all hover:opacity-80"
          style={{
            background: `${config.color}20`,
            color: config.color,
            border: isPinned ? `1.5px solid ${config.color}` : "1.5px solid transparent",
          }}
          title={`Mode: ${config.label}${isPinned ? " (Pinned)" : ""}\nApp: ${context.activeAppName || context.activeApp || "Unknown"}`}
        >
          {config.icon}
          <span>{config.label}</span>
          {isPinned && (
            <svg className="w-3 h-3 ml-0.5" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clipRule="evenodd" />
            </svg>
          )}
        </button>

        {/* Mode Selector Menu */}
        {showMenu && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setShowMenu(false)} />
            <div
              className="absolute top-full left-0 mt-1 py-1 rounded-lg shadow-lg z-50 min-w-[140px]"
              style={{ background: "var(--color-bg-secondary)", border: "1px solid var(--color-border)" }}
            >
              {(Object.keys(MODE_CONFIG) as Mode[]).map((mode) => {
                const modeConfig = MODE_CONFIG[mode];
                const isActive = context.mode === mode;
                const isDetected = context.detectedMode === mode;

                return (
                  <button
                    key={mode}
                    onClick={() => {
                      if (isPinned && context.pinnedMode === mode) {
                        unpinMode();
                      } else {
                        pinMode(mode);
                      }
                      setShowMenu(false);
                    }}
                    className="w-full flex items-center gap-2 px-3 py-1.5 text-xs transition-colors hover:bg-[var(--color-bg-tertiary)]"
                    style={{ color: isActive ? modeConfig.color : "var(--color-text-secondary)" }}
                  >
                    <span style={{ color: modeConfig.color }}>{modeConfig.icon}</span>
                    <span className="flex-1 text-left">{modeConfig.label}</span>
                    {isActive && isPinned && (
                      <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                        <path fillRule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clipRule="evenodd" />
                      </svg>
                    )}
                    {isDetected && !isPinned && (
                      <span className="text-[10px] opacity-50">auto</span>
                    )}
                  </button>
                );
              })}
              {isPinned && (
                <>
                  <div className="my-1 border-t" style={{ borderColor: "var(--color-border)" }} />
                  <button
                    onClick={() => {
                      unpinMode();
                      setShowMenu(false);
                    }}
                    className="w-full flex items-center gap-2 px-3 py-1.5 text-xs transition-colors hover:bg-[var(--color-bg-tertiary)]"
                    style={{ color: "var(--color-text-tertiary)" }}
                  >
                    <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M8 11V7a4 4 0 118 0m-4 8v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2z" />
                    </svg>
                    <span>Auto-detect</span>
                  </button>
                </>
              )}
            </div>
          </>
        )}
      </div>

      {/* Mode Change Toast */}
      {showToast && (
        <div
          className="fixed top-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-2 px-4 py-2 rounded-full shadow-lg animate-slide-down"
          style={{
            background: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          <span style={{ color: config.color }}>{config.icon}</span>
          <span className="text-sm" style={{ color: "var(--color-text-primary)" }}>
            Switched to {config.label} mode
          </span>
        </div>
      )}
    </>
  );
}
