import { useStt, type ModelInfo } from "../../hooks/use-stt";
import { SectionHeader, formatBytes } from "./shared";

interface ModelCardProps {
  model: ModelInfo;
  currentModel: string | null;
  downloadProgress: Record<string, number>;
  onDownload: () => void;
  onCancel: () => void;
  onLoad: () => void;
  onUnload: () => void;
  loadingModel: string | null;
}

function ModelCard({
  model,
  currentModel,
  downloadProgress,
  onDownload,
  onCancel,
  onLoad,
  onUnload,
  loadingModel,
}: ModelCardProps) {
  const isDownloading = model.name in downloadProgress;
  const isActive = currentModel === model.name;
  const isLoading = loadingModel === model.name;
  const isAnyLoading = loadingModel !== null;
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
            {isActive && <span className="badge badge-live">Active</span>}
          </div>
          <p className="text-sm mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
            {formatBytes(model.size_bytes)}
          </p>
        </div>

        <div className="flex items-center gap-2">
          {model.is_downloaded ? (
            isActive ? (
              <button onClick={onUnload} disabled={isAnyLoading} className="btn-secondary text-sm">
                Unload
              </button>
            ) : (
              <button onClick={onLoad} disabled={isAnyLoading} className="btn-primary text-sm">
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
              disabled={isAnyLoading}
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

export function SpeechModelsSection() {
  const {
    models,
    currentModel,
    loadingModel,
    downloadProgress,
    downloadModel,
    cancelDownload,
    loadModel,
    unloadModel,
  } = useStt();

  return (
    <section>
      <SectionHeader>Speech Models</SectionHeader>
      <div className="space-y-3">
        {models.map((model) => (
          <ModelCard
            key={model.name}
            model={model}
            currentModel={currentModel}
            downloadProgress={downloadProgress}
            onDownload={() => downloadModel(model.name)}
            onCancel={() => cancelDownload(model.name)}
            onLoad={() => loadModel(model.name)}
            onUnload={() => unloadModel()}
            loadingModel={loadingModel}
          />
        ))}
      </div>
      {models.length === 0 && (
        <p style={{ color: "var(--color-text-tertiary)" }}>No models available</p>
      )}
    </section>
  );
}
