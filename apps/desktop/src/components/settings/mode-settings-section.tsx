import { useEffect, ReactNode } from "react";
import { useContextStore, Mode } from "../../stores/context-store";
import { SectionHeader } from "./shared";

const MODE_CONFIG: Record<
  Mode,
  { label: string; color: string; description: string; icon: ReactNode }
> = {
  Meeting: {
    label: "Meeting",
    color: "#ef4444",
    description: "Active during video calls (Zoom, Teams, Meet)",
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
      </svg>
    ),
  },
  Dev: {
    label: "Dev",
    color: "#3b82f6",
    description: "Active in code editors (VS Code, Xcode, terminals)",
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
      </svg>
    ),
  },
  Writer: {
    label: "Writer",
    color: "#8b5cf6",
    description: "Active in writing apps (Notes, Word, Pages)",
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
      </svg>
    ),
  },
  Global: {
    label: "Global",
    color: "#6b7280",
    description: "Default mode for general use",
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    ),
  },
};

function ModeCard({ mode, isActive, isPinned, isDetected, onSelect }: {
  mode: Mode;
  isActive: boolean;
  isPinned: boolean;
  isDetected: boolean;
  onSelect: () => void;
}) {
  const config = MODE_CONFIG[mode];

  return (
    <button
      onClick={onSelect}
      className="w-full p-3 rounded-lg text-left transition-all"
      style={{
        background: isActive ? `${config.color}15` : "var(--color-bg-tertiary)",
        border: isActive ? `1.5px solid ${config.color}` : "1.5px solid transparent",
      }}
    >
      <div className="flex items-center gap-2 mb-1">
        <span style={{ color: config.color }}>{config.icon}</span>
        <span
          className="font-medium text-sm"
          style={{ color: isActive ? config.color : "var(--color-text-primary)" }}
        >
          {config.label}
        </span>
        {isPinned && (
          <svg className="w-3.5 h-3.5 ml-auto" fill={config.color} viewBox="0 0 20 20">
            <path fillRule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clipRule="evenodd" />
          </svg>
        )}
        {isDetected && !isPinned && (
          <span
            className="text-[10px] ml-auto px-1.5 py-0.5 rounded"
            style={{ background: `${config.color}20`, color: config.color }}
          >
            detected
          </span>
        )}
      </div>
      <div className="text-xs" style={{ color: "var(--color-text-tertiary)" }}>
        {config.description}
      </div>
    </button>
  );
}

function ModeSettingsCard() {
  const { context, isLoading, initialize, pinMode, unpinMode } = useContextStore();

  useEffect(() => {
    initialize();
  }, [initialize]);

  const isPinned = context.pinnedMode !== null;
  const currentConfig = MODE_CONFIG[context.mode];

  return (
    <div className="card p-4 space-y-4" style={{ background: "var(--color-bg-secondary)" }}>
      {/* Current Status */}
      <div className="flex items-center justify-between">
        <div>
          <div className="font-medium text-sm" style={{ color: "var(--color-text-primary)" }}>
            Current Mode
          </div>
          <div className="text-xs mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
            {isPinned ? "Manually pinned" : "Auto-detected based on active app"}
          </div>
        </div>
        <div
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium"
          style={{
            background: `${currentConfig.color}20`,
            color: currentConfig.color,
          }}
        >
          {currentConfig.icon}
          <span>{currentConfig.label}</span>
          {isPinned && (
            <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clipRule="evenodd" />
            </svg>
          )}
        </div>
      </div>

      {/* Active App Info */}
      {context.activeApp && (
        <div
          className="text-xs px-3 py-2 rounded"
          style={{
            background: "var(--color-bg-tertiary)",
            color: "var(--color-text-secondary)"
          }}
        >
          <span style={{ color: "var(--color-text-tertiary)" }}>Active app: </span>
          {context.activeAppName || context.activeApp}
          {context.isMeeting && (
            <span
              className="ml-2 px-1.5 py-0.5 rounded text-[10px]"
              style={{ background: "#ef444420", color: "#ef4444" }}
            >
              In Meeting
            </span>
          )}
        </div>
      )}

      {/* Mode Selector */}
      <div className="space-y-2">
        <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
          Select mode
        </div>
        <div className="grid grid-cols-2 gap-2">
          {(Object.keys(MODE_CONFIG) as Mode[]).map((mode) => (
            <ModeCard
              key={mode}
              mode={mode}
              isActive={context.mode === mode}
              isPinned={context.pinnedMode === mode}
              isDetected={context.detectedMode === mode && context.pinnedMode === null}
              onSelect={() => {
                if (isPinned && context.pinnedMode === mode) {
                  unpinMode();
                } else {
                  pinMode(mode);
                }
              }}
            />
          ))}
        </div>
      </div>

      {/* Auto-detect Button */}
      {isPinned && (
        <button
          onClick={() => unpinMode()}
          disabled={isLoading}
          className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded text-sm transition-colors"
          style={{
            background: "var(--color-bg-tertiary)",
            color: "var(--color-text-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M8 11V7a4 4 0 118 0m-4 8v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2z" />
          </svg>
          Enable Auto-Detect
        </button>
      )}

      {/* Mode Explanation */}
      <div
        className="text-xs p-3 rounded"
        style={{
          background: "var(--color-bg-tertiary)",
          color: "var(--color-text-tertiary)"
        }}
      >
        <strong style={{ color: "var(--color-text-secondary)" }}>How it works:</strong>
        <ul className="mt-1 ml-4 list-disc space-y-1">
          <li>Modes filter which voice tools are available to FunctionGemma</li>
          <li>Auto-detect switches modes based on the active application</li>
          <li>Pin a mode to disable auto-switching</li>
        </ul>
      </div>
    </div>
  );
}

export function ModeSettingsSection() {
  return (
    <section>
      <SectionHeader>Context Modes</SectionHeader>
      <ModeSettingsCard />
    </section>
  );
}
