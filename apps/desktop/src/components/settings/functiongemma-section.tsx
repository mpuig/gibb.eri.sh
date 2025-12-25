import { useFunctionGemma } from "../../hooks/use-functiongemma";
import { SectionHeader, formatBytes } from "./shared";

export function FunctionGemmaSection() {
  const {
    models,
    currentModel,
    downloadProgress,
    downloadFile,
    error,
    isLoading,
    downloadModel,
    cancelDownload,
    loadModel,
    unloadModel,
  } = useFunctionGemma();

  return (
    <section>
      <SectionHeader>FunctionGemma Model</SectionHeader>
      <div className="card p-4 space-y-2" style={{ background: "var(--color-bg-secondary)" }}>
        <div className="flex items-center justify-between">
          <div>
            <div className="font-medium text-sm" style={{ color: "var(--color-text-primary)" }}>
              FunctionGemma (ONNX){currentModel ? ` — Loaded: ${currentModel}` : ""}
            </div>
            <div className="text-xs mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
              Optional local model to propose tool calls from STT commits.
            </div>
          </div>
          {currentModel && (
            <button className="btn-secondary text-sm" disabled={isLoading} onClick={unloadModel}>
              Unload
            </button>
          )}
        </div>

        {error && (
          <div className="text-xs" style={{ color: "var(--color-danger)" }}>
            {error}
          </div>
        )}

        <div className="space-y-2">
          {models.map((m) => {
            const progress = downloadProgress[m.variant];
            const file = downloadFile[m.variant];
            const isLoaded = currentModel === m.variant;
            const isDownloading = m.is_downloading || typeof progress === "number";

            return (
              <div
                key={m.variant}
                className="flex items-center justify-between gap-3 rounded-lg px-3 py-2"
                style={{
                  background: "var(--color-bg-primary)",
                  border: "1px solid var(--color-border)",
                }}
              >
                <div className="min-w-0">
                  <div className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
                    {m.variant}
                  </div>
                  <div className="text-xs mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
                    {formatBytes(m.size_bytes)}
                    {isDownloading && typeof progress === "number"
                      ? ` — Downloading ${progress}%${file ? ` (${file})` : ""}`
                      : ""}
                    {isLoaded ? " — Active" : ""}
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  {!m.is_downloaded ? (
                    isDownloading ? (
                      <button
                        className="btn-ghost text-xs"
                        disabled={isLoading}
                        style={{ color: "var(--color-danger)" }}
                        onClick={() => cancelDownload(m.variant)}
                      >
                        Cancel
                      </button>
                    ) : (
                      <button
                        className="btn-primary text-sm"
                        disabled={isLoading}
                        onClick={() => downloadModel(m.variant)}
                      >
                        Download
                      </button>
                    )
                  ) : isLoaded ? (
                    <button className="btn-secondary text-sm" disabled={isLoading} onClick={unloadModel}>
                      Unload
                    </button>
                  ) : (
                    <button
                      className="btn-primary text-sm"
                      disabled={isLoading}
                      onClick={() => loadModel(m.variant)}
                    >
                      Load
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
