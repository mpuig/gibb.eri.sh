import { useMemo } from "react";
import { useSmartTurn, type TurnModelInfo } from "../../hooks/use-smart-turn";
import { SectionHeader, formatBytes } from "./shared";

interface ModelCardProps {
  model: TurnModelInfo;
  currentModel: string | null;
  downloadProgress: Record<string, number>;
  onDownload: () => void;
  onCancel: () => void;
  onLoad: () => void;
  onUnload: () => void;
  isLoading: boolean;
}

function ModelCard({
  model,
  currentModel,
  downloadProgress,
  onDownload,
  onCancel,
  onLoad,
  onUnload,
  isLoading,
}: ModelCardProps) {
  const isDownloading = model.name in downloadProgress;
  const isActive = currentModel === model.name;
  const progress = downloadProgress[model.name] ?? 0;

  return (
    <div
      className="card p-4 transition-all duration-150"
      style={{
        borderColor: isActive ? "var(--color-accent)" : "var(--color-border)",
        background: isActive ? "rgba(10, 132, 255, 0.08)" : "var(--color-bg-secondary)",
      }}
    >
      <div className="flex justify-between items-start gap-4">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="font-medium" style={{ color: "var(--color-text-primary)" }}>
              {model.name}
            </h3>
            {isActive && <span className="badge badge-live">Loaded</span>}
          </div>
          <p className="text-sm mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
            {formatBytes(model.size_bytes)}
          </p>
        </div>

        <div className="flex items-center gap-2">
          {model.is_downloaded ? (
            isActive ? (
              <button onClick={onUnload} disabled={isLoading} className="btn-secondary text-sm">
                Unload
              </button>
            ) : (
              <button onClick={onLoad} disabled={isLoading} className="btn-primary text-sm">
                {isLoading ? "Loading..." : "Load"}
              </button>
            )
          ) : isDownloading ? (
            <div className="flex items-center gap-3">
              <div className="w-24">
                <div className="progress">
                  <div className="progress-bar" style={{ width: `${progress}%` }} />
                </div>
                <span className="text-xs mt-1 block" style={{ color: "var(--color-text-tertiary)" }}>
                  {progress}%
                </span>
              </div>
              <button
                onClick={onCancel}
                className="btn-ghost text-sm"
                style={{ color: "var(--color-danger)" }}
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              onClick={onDownload}
              disabled={isLoading}
              className="btn-primary text-sm"
              style={{ background: "var(--color-success)" }}
            >
              Download
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function SmartTurnSection() {
  const {
    models,
    currentModel,
    settings,
    downloadProgress,
    lastPrediction,
    isLoading,
    error,
    downloadModel,
    cancelDownload,
    loadModel,
    unloadModel,
    updateSettings,
  } = useSmartTurn();

  const model = useMemo(() => models[0], [models]);

  return (
    <section>
      <SectionHeader>Turn Detection</SectionHeader>

      <div className="space-y-3">
        {model ? (
          <ModelCard
            model={model}
            currentModel={currentModel}
            downloadProgress={downloadProgress}
            onDownload={() => downloadModel(model.name)}
            onCancel={() => cancelDownload(model.name)}
            onLoad={() => loadModel(model.name)}
            onUnload={() => unloadModel()}
            isLoading={isLoading}
          />
        ) : (
          <p style={{ color: "var(--color-text-tertiary)" }}>No turn models available</p>
        )}

        <div className="card p-4" style={{ background: "var(--color-bg-secondary)" }}>
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium" style={{ color: "var(--color-text-primary)" }}>
                Enable Smart Turn
              </div>
              <div className="text-sm mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
                Uses a small on-device model at VAD pauses to reduce premature commits.
              </div>
            </div>
            <label className="inline-flex items-center gap-2">
              <input
                type="checkbox"
                checked={settings.enabled}
                disabled={isLoading}
                onChange={(e) => updateSettings(e.target.checked, settings.threshold)}
              />
            </label>
          </div>

          <div className="mt-4">
            <div className="flex items-center justify-between">
              <div className="text-sm" style={{ color: "var(--color-text-primary)" }}>
                Endpoint threshold
              </div>
              <div className="text-sm tabular-nums" style={{ color: "var(--color-text-tertiary)" }}>
                {settings.threshold.toFixed(2)}
              </div>
            </div>
            <input
              className="w-full mt-2"
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={settings.threshold}
              disabled={isLoading}
              onChange={(e) => updateSettings(settings.enabled, Number(e.target.value))}
            />
            <div className="text-xs mt-1" style={{ color: "var(--color-text-tertiary)" }}>
              Lower commits sooner; higher waits longer.
            </div>
          </div>

          {lastPrediction && (
            <div className="mt-4 text-xs" style={{ color: "var(--color-text-tertiary)" }}>
              Last prediction: {lastPrediction.probability.toFixed(3)}{" "}
              {lastPrediction.is_complete ? "(end)" : "(continue)"}
            </div>
          )}
        </div>

        {error && (
          <div className="text-sm" style={{ color: "var(--color-danger)" }}>
            {error}
          </div>
        )}
      </div>
    </section>
  );
}

