import { useEffect, useMemo } from "react";
import { useStt } from "../hooks/use-stt";
import { useSmartTurn } from "../hooks/use-smart-turn";

interface SettingsSheetProps {
  isOpen: boolean;
  onClose: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function SettingsSheet({ isOpen, onClose }: SettingsSheetProps) {
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

  const {
    models: turnModels,
    currentModel: currentTurnModel,
    settings: turnSettings,
    downloadProgress: turnDownloadProgress,
    isLoading: turnIsLoading,
    downloadModel: downloadTurnModel,
    cancelDownload: cancelTurnDownload,
    loadModel: loadTurnModel,
    unloadModel: unloadTurnModel,
    updateSettings: updateTurnSettings,
  } = useSmartTurn();

  const turnModel = useMemo(() => turnModels[0], [turnModels]);

  // Close on escape key
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    if (isOpen) {
      document.addEventListener("keydown", handleEscape);
    }
    return () => document.removeEventListener("keydown", handleEscape);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Sheet */}
      <div
        className="absolute bottom-0 left-0 right-0 rounded-t-2xl overflow-hidden animate-in"
        style={{
          background: "var(--color-bg-primary)",
          maxHeight: "80vh",
          animation: "slideUp 0.2s ease-out",
        }}
      >
        {/* Handle */}
        <div className="flex justify-center py-3">
          <div
            className="w-10 h-1 rounded-full"
            style={{ background: "var(--color-border)" }}
          />
        </div>

        {/* Header */}
        <div className="flex items-center justify-between px-6 pb-4">
          <h2 className="text-lg font-semibold" style={{ color: "var(--color-text-primary)" }}>
            Settings
          </h2>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-full flex items-center justify-center"
            style={{ background: "var(--color-bg-tertiary)" }}
          >
            <svg className="w-4 h-4" style={{ color: "var(--color-text-tertiary)" }} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="px-6 pb-8 overflow-auto" style={{ maxHeight: "calc(80vh - 100px)" }}>
          {/* Models Section */}
          <div className="mb-6">
            <h3 className="text-sm font-medium mb-3" style={{ color: "var(--color-text-secondary)" }}>
              Speech Model
            </h3>
            <div className="space-y-2">
              {models.map((model) => {
                const isDownloading = model.name in downloadProgress;
                const isActive = currentModel === model.name;
                const isLoading = loadingModel === model.name;
                const isAnyLoading = loadingModel !== null;
                const progress = downloadProgress[model.name] ?? 0;

                return (
                  <div
                    key={model.name}
                    className="p-3 rounded-xl transition-all"
                    style={{
                      background: isActive ? "rgba(10, 132, 255, 0.1)" : "var(--color-bg-secondary)",
                      border: isActive ? "1px solid var(--color-accent)" : "1px solid transparent",
                    }}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-sm" style={{ color: "var(--color-text-primary)" }}>
                            {model.name}
                          </span>
                          {isActive && (
                            <span
                              className="text-xs px-1.5 py-0.5 rounded"
                              style={{ background: "var(--color-accent)", color: "white" }}
                            >
                              Active
                            </span>
                          )}
                        </div>
                        <span className="text-xs" style={{ color: "var(--color-text-quaternary)" }}>
                          {formatBytes(model.size_bytes)}
                        </span>
                      </div>

                      <div className="flex items-center gap-2">
                        {model.is_downloaded ? (
                          isActive ? (
                            <button
                              onClick={() => unloadModel()}
                              disabled={isAnyLoading}
                              className="text-xs px-3 py-1.5 rounded-lg transition-colors"
                              style={{
                                background: "var(--color-bg-tertiary)",
                                color: "var(--color-text-secondary)",
                                opacity: isAnyLoading ? 0.5 : 1,
                              }}
                            >
                              Unload
                            </button>
                          ) : (
                            <button
                              onClick={() => loadModel(model.name)}
                              disabled={isAnyLoading}
                              className="text-xs px-3 py-1.5 rounded-lg transition-colors"
                              style={{
                                background: "var(--color-accent)",
                                color: "white",
                                opacity: isAnyLoading ? 0.5 : 1,
                              }}
                            >
                              {isLoading ? "Loading..." : "Load"}
                            </button>
                          )
                        ) : isDownloading ? (
                          <div className="flex items-center gap-2">
                            <div className="w-16">
                              <div
                                className="h-1.5 rounded-full overflow-hidden"
                                style={{ background: "var(--color-bg-tertiary)" }}
                              >
                                <div
                                  className="h-full rounded-full transition-all"
                                  style={{
                                    width: `${progress}%`,
                                    background: "var(--color-accent)",
                                  }}
                                />
                              </div>
                            </div>
                            <button
                              onClick={() => cancelDownload(model.name)}
                              className="text-xs"
                              style={{ color: "var(--color-danger)" }}
                            >
                              Cancel
                            </button>
                          </div>
                        ) : (
                          <button
                            onClick={() => downloadModel(model.name)}
                            disabled={isAnyLoading}
                            className="text-xs px-3 py-1.5 rounded-lg transition-colors"
                            style={{
                              background: "var(--color-success)",
                              color: "white",
                              opacity: isAnyLoading ? 0.5 : 1,
                            }}
                          >
                            Download
                          </button>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Turn Detection Section */}
          <div className="mb-6">
            <h3 className="text-sm font-medium mb-3" style={{ color: "var(--color-text-secondary)" }}>
              Turn Detection
            </h3>

            {/* Enable Toggle */}
            <div
              className="p-3 rounded-xl mb-2"
              style={{ background: "var(--color-bg-secondary)" }}
            >
              <div className="flex items-center justify-between">
                <div>
                  <span className="font-medium text-sm" style={{ color: "var(--color-text-primary)" }}>
                    Smart Turn Detection
                  </span>
                  <p className="text-xs mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
                    Detects end of speech to reduce premature commits
                  </p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={turnSettings.enabled}
                    onChange={(e) => updateTurnSettings(e.target.checked, turnSettings.threshold)}
                    className="sr-only peer"
                  />
                  <div
                    className="w-9 h-5 rounded-full peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all"
                    style={{
                      background: turnSettings.enabled ? "var(--color-accent)" : "var(--color-bg-tertiary)",
                    }}
                  />
                </label>
              </div>

              {/* Threshold Slider */}
              {turnSettings.enabled && (
                <div className="mt-3 pt-3 border-t" style={{ borderColor: "var(--color-border)" }}>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-xs" style={{ color: "var(--color-text-secondary)" }}>
                      Sensitivity
                    </span>
                    <span className="text-xs tabular-nums" style={{ color: "var(--color-text-tertiary)" }}>
                      {turnSettings.threshold.toFixed(2)}
                    </span>
                  </div>
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.01}
                    value={turnSettings.threshold}
                    onChange={(e) => updateTurnSettings(turnSettings.enabled, Number(e.target.value))}
                    className="w-full h-1.5 rounded-full appearance-none cursor-pointer"
                    style={{ background: "var(--color-bg-tertiary)" }}
                  />
                  <div className="flex justify-between text-xs mt-1" style={{ color: "var(--color-text-quaternary)" }}>
                    <span>Faster</span>
                    <span>More accurate</span>
                  </div>
                </div>
              )}
            </div>

            {/* Turn Model */}
            {turnModel && (
              <div
                className="p-3 rounded-xl"
                style={{ background: "var(--color-bg-secondary)" }}
              >
                <div className="flex items-center justify-between">
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
                        Turn detection model
                      </span>
                      {currentTurnModel && (
                        <span
                          className="text-xs px-1.5 py-0.5 rounded"
                          style={{ background: "var(--color-accent)", color: "white" }}
                        >
                          Active
                        </span>
                      )}
                    </div>
                    <span className="text-xs" style={{ color: "var(--color-text-quaternary)" }}>
                      {formatBytes(turnModel.size_bytes)}
                    </span>
                  </div>
                  {turnModel.is_downloaded ? (
                    currentTurnModel ? (
                      <button
                        onClick={() => unloadTurnModel()}
                        disabled={turnIsLoading}
                        className="text-xs px-3 py-1.5 rounded-lg transition-colors"
                        style={{
                          background: "var(--color-bg-tertiary)",
                          color: "var(--color-text-secondary)",
                          opacity: turnIsLoading ? 0.5 : 1,
                        }}
                      >
                        Unload
                      </button>
                    ) : (
                      <button
                        onClick={() => loadTurnModel(turnModel.name)}
                        disabled={turnIsLoading}
                        className="text-xs px-3 py-1.5 rounded-lg transition-colors"
                        style={{
                          background: "var(--color-accent)",
                          color: "white",
                          opacity: turnIsLoading ? 0.5 : 1,
                        }}
                      >
                        {turnIsLoading ? "Loading..." : "Load"}
                      </button>
                    )
                  ) : turnModel.name in turnDownloadProgress ? (
                    <div className="flex items-center gap-2">
                      <div className="w-16">
                        <div className="h-1.5 rounded-full overflow-hidden" style={{ background: "var(--color-bg-tertiary)" }}>
                          <div
                            className="h-full rounded-full"
                            style={{ width: `${turnDownloadProgress[turnModel.name]}%`, background: "var(--color-accent)" }}
                          />
                        </div>
                      </div>
                      <button
                        onClick={() => cancelTurnDownload(turnModel.name)}
                        className="text-xs"
                        style={{ color: "var(--color-danger)" }}
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <button
                      onClick={() => downloadTurnModel(turnModel.name)}
                      className="text-xs px-3 py-1.5 rounded-lg"
                      style={{ background: "var(--color-success)", color: "white" }}
                    >
                      Download
                    </button>
                  )}
                </div>
              </div>
            )}
          </div>

          {/* Version */}
          <div className="text-center pt-4 border-t" style={{ borderColor: "var(--color-border)" }}>
            <span className="text-xs" style={{ color: "var(--color-text-quaternary)" }}>
              Gibberish v0.1.0
            </span>
          </div>
        </div>
      </div>

      <style>{`
        @keyframes slideUp {
          from {
            transform: translateY(100%);
          }
          to {
            transform: translateY(0);
          }
        }
      `}</style>
    </div>
  );
}
