import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useStt, type ModelInfo } from "../hooks/use-stt";
import { useSessions } from "../hooks/use-sessions";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <h2
      className="text-xs font-semibold uppercase tracking-wider mb-3"
      style={{ color: "var(--color-text-tertiary)" }}
    >
      {children}
    </h2>
  );
}

function ModelCard({ model, currentModel, downloadProgress, onDownload, onCancel, onLoad, onUnload, isLoading }: {
  model: ModelInfo;
  currentModel: string | null;
  downloadProgress: Record<string, number>;
  onDownload: () => void;
  onCancel: () => void;
  onLoad: () => void;
  onUnload: () => void;
  isLoading: boolean;
}) {
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
            {isActive && (
              <span className="badge badge-live">Active</span>
            )}
          </div>
          <p className="text-sm mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
            {formatBytes(model.size_bytes)}
          </p>
        </div>

        <div className="flex items-center gap-2">
          {model.is_downloaded ? (
            isActive ? (
              <button
                onClick={onUnload}
                disabled={isLoading}
                className="btn-secondary text-sm"
              >
                Unload
              </button>
            ) : (
              <button
                onClick={onLoad}
                disabled={isLoading}
                className="btn-primary text-sm"
              >
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
              <button onClick={onCancel} className="btn-ghost text-sm" style={{ color: "var(--color-danger)" }}>
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


function StorageSettings() {
  const { sessions, loadSessions } = useSessions();
  const [isClearing, setIsClearing] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);

  const handleClearAllSessions = async () => {
    setIsClearing(true);
    try {
      for (const session of sessions) {
        await invoke("plugin:gibberish-stt|delete_session", { id: session.id });
      }
      await loadSessions();
    } catch (err) {
      console.error("Failed to clear sessions:", err);
    } finally {
      setIsClearing(false);
      setShowConfirm(false);
    }
  };

  return (
    <div className="space-y-3">
      <div
        className="card p-4 flex items-center justify-between"
        style={{ background: "var(--color-bg-secondary)" }}
      >
        <div>
          <div className="font-medium text-sm" style={{ color: "var(--color-text-primary)" }}>
            Saved Sessions
          </div>
          <div className="text-xs mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
            {sessions.length} recording{sessions.length !== 1 ? "s" : ""} stored locally
          </div>
        </div>
        {showConfirm ? (
          <div className="flex items-center gap-2">
            <span className="text-xs" style={{ color: "var(--color-text-tertiary)" }}>Delete all?</span>
            <button
              onClick={handleClearAllSessions}
              disabled={isClearing}
              className="btn-danger text-xs px-2 py-1"
            >
              {isClearing ? "..." : "Yes"}
            </button>
            <button
              onClick={() => setShowConfirm(false)}
              className="btn-secondary text-xs px-2 py-1"
            >
              No
            </button>
          </div>
        ) : (
          <button
            onClick={() => setShowConfirm(true)}
            disabled={sessions.length === 0}
            className="btn-secondary text-sm"
            style={{ opacity: sessions.length === 0 ? 0.5 : 1 }}
          >
            Clear All
          </button>
        )}
      </div>
    </div>
  );
}

function AboutSection() {
  return (
    <div className="flex items-start gap-4">
      <div
        className="w-14 h-14 rounded-2xl flex items-center justify-center flex-shrink-0"
        style={{ background: "linear-gradient(135deg, var(--color-accent), #5856d6)" }}
      >
        <span className="text-2xl font-bold text-white">G</span>
      </div>
      <div>
        <h3 className="font-semibold" style={{ color: "var(--color-text-primary)" }}>
          gibb.eri.sh
        </h3>
        <p className="text-sm" style={{ color: "var(--color-text-tertiary)" }}>
          Version 0.1.0
        </p>
        <p className="text-sm mt-2" style={{ color: "var(--color-text-tertiary)" }}>
          Local, private speech-to-text. All processing happens on your device.
        </p>
        <a
          href="https://github.com/mpuig/gibb.eri.sh"
          target="_blank"
          rel="noopener noreferrer"
          className="text-sm mt-1 inline-block"
          style={{ color: "var(--color-accent)" }}
        >
          github.com/mpuig/gibb.eri.sh
        </a>
      </div>
    </div>
  );
}

type ActionRouterSettings = {
  enabled: boolean;
  auto_run_read_only: boolean;
  default_lang: string;
};

type FunctionGemmaModelInfo = {
  variant: string;
  is_downloaded: boolean;
  size_bytes: number;
  is_downloading: boolean;
};

export function Settings() {
  const {
    models,
    currentModel,
    isLoading,
    downloadProgress,
    downloadModel,
    cancelDownload,
    loadModel,
    unloadModel,
  } = useStt();

  const [routerSettings, setRouterSettings] = useState<ActionRouterSettings | null>(null);
  const [fgModels, setFgModels] = useState<FunctionGemmaModelInfo[]>([]);
  const [fgCurrent, setFgCurrent] = useState<string | null>(null);
  const [fgDownloadProgress, setFgDownloadProgress] = useState<Record<string, number>>({});
  const [fgDownloadFile, setFgDownloadFile] = useState<Record<string, string>>({});
  const [fgError, setFgError] = useState<string | null>(null);
  const [isLoadingActions, setIsLoadingActions] = useState(false);

  const refreshFunctionGemma = useCallback(async () => {
    const [models, current] = await Promise.all([
      invoke<FunctionGemmaModelInfo[]>("plugin:gibberish-tools|list_functiongemma_models"),
      invoke<string | null>("plugin:gibberish-tools|get_current_functiongemma_model"),
    ]);
    setFgModels(models);
    setFgCurrent(current);
  }, []);

  useEffect(() => {
    let mounted = true;
    const unlisteners: (() => void)[] = [];

    const load = async () => {
      try {
        const [settings] = await Promise.all([
          invoke<ActionRouterSettings>("plugin:gibberish-tools|get_action_router_settings"),
        ]);
        if (mounted) {
          setRouterSettings(settings);
          setFgError(null);
        }
        await refreshFunctionGemma();
      } catch (err) {
        console.error("Failed to load action router settings:", err);
      }
    };
    load();

    // Live updates while downloading/loading.
    listen<{
      variant: string;
      file: string;
      progress: number;
      downloaded_bytes: number;
      total_bytes: number;
      file_downloaded_bytes: number;
      file_total_bytes: number;
    }>("tools:functiongemma_download_progress", (event) => {
      if (!mounted) return;
      setFgDownloadProgress((prev) => ({ ...prev, [event.payload.variant]: event.payload.progress }));
      setFgDownloadFile((prev) => ({ ...prev, [event.payload.variant]: event.payload.file }));
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    listen<{ variant: string }>("tools:functiongemma_download_complete", async () => {
      if (!mounted) return;
      await refreshFunctionGemma();
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    listen<{ variant: string; error: string }>("tools:functiongemma_download_error", async (event) => {
      if (!mounted) return;
      setFgError(event.payload.error);
      await refreshFunctionGemma();
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    listen<{ variant: string }>("tools:functiongemma_loaded", async () => {
      if (!mounted) return;
      await refreshFunctionGemma();
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    return () => {
      mounted = false;
      unlisteners.forEach((u) => u());
    };
  }, []);

  const updateRouterSettings = async (updates: Partial<ActionRouterSettings>) => {
    if (!routerSettings) return;
    setIsLoadingActions(true);
    try {
      const next = await invoke<ActionRouterSettings>(
        "plugin:gibberish-tools|set_action_router_settings",
        {
          enabled: updates.enabled,
          autoRunReadOnly: updates.auto_run_read_only,
          defaultLang: updates.default_lang,
        }
      );
      setRouterSettings(next);
    } catch (err) {
      console.error("Failed to update action router settings:", err);
    } finally {
      setIsLoadingActions(false);
    }
  };

  const downloadFunctionGemma = async (variant: string) => {
    setIsLoadingActions(true);
    setFgError(null);
    setFgDownloadProgress((prev) => ({ ...prev, [variant]: 0 }));
    try {
      // Start download and keep the UI reactive while we receive progress events.
      const p = invoke<string>("plugin:gibberish-tools|download_functiongemma_model", { variant });
      setFgModels((prev) =>
        prev.map((m) => (m.variant === variant ? { ...m, is_downloading: true } : m))
      );
      await p;
      await refreshFunctionGemma();
    } catch (err) {
      console.error("Failed to download FunctionGemma:", err);
      setFgError(String(err));
    } finally {
      setIsLoadingActions(false);
    }
  };

  const cancelFunctionGemmaDownload = async (variant: string) => {
    setIsLoadingActions(true);
    try {
      await invoke<boolean>("plugin:gibberish-tools|cancel_functiongemma_download", { variant });
      await refreshFunctionGemma();
      setFgDownloadProgress((prev) => {
        const { [variant]: _, ...rest } = prev;
        return rest;
      });
      setFgDownloadFile((prev) => {
        const { [variant]: _, ...rest } = prev;
        return rest;
      });
    } catch (err) {
      console.error("Failed to cancel FunctionGemma download:", err);
    } finally {
      setIsLoadingActions(false);
    }
  };

  const loadFunctionGemma = async (variant: string) => {
    setIsLoadingActions(true);
    setFgError(null);
    try {
      await invoke("plugin:gibberish-tools|load_functiongemma_model", { variant });
      await refreshFunctionGemma();
    } catch (err) {
      console.error("Failed to load FunctionGemma:", err);
      setFgError(String(err));
    } finally {
      setIsLoadingActions(false);
    }
  };

  const unloadFunctionGemma = async () => {
    setIsLoadingActions(true);
    try {
      await invoke("plugin:gibberish-tools|unload_functiongemma_model");
      await refreshFunctionGemma();
    } catch (err) {
      console.error("Failed to unload FunctionGemma:", err);
    } finally {
      setIsLoadingActions(false);
    }
  };

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-8">
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
              isLoading={isLoading}
            />
          ))}
        </div>
        {models.length === 0 && (
          <p style={{ color: "var(--color-text-tertiary)" }}>No models available</p>
        )}
      </section>

      <section>
        <SectionHeader>Actions</SectionHeader>
        <div className="space-y-3">
          <div
            className="card p-4 space-y-3"
            style={{ background: "var(--color-bg-secondary)" }}
          >
            <div className="flex items-center justify-between">
              <div>
                <div className="font-medium text-sm" style={{ color: "var(--color-text-primary)" }}>
                  Action Router
                </div>
                <div className="text-xs mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
                  Runs alongside live transcription and can call read-only tools (like Wikipedia).
                </div>
              </div>
              <button
                className="btn-secondary text-sm"
                disabled={isLoadingActions || !routerSettings}
                onClick={() => updateRouterSettings({ enabled: !(routerSettings?.enabled ?? true) })}
              >
                {routerSettings?.enabled ? "Disable" : "Enable"}
              </button>
            </div>

            <div className="flex items-center justify-between">
              <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
                Auto-run read-only actions
              </div>
              <button
                className="btn-secondary text-sm"
                disabled={isLoadingActions || !routerSettings}
                onClick={() =>
                  updateRouterSettings({
                    auto_run_read_only: !(routerSettings?.auto_run_read_only ?? true),
                  })
                }
              >
                {routerSettings?.auto_run_read_only ? "On" : "Off"}
              </button>
            </div>

            <div className="flex items-center justify-between gap-3">
              <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
                Default Wikipedia language
              </div>
              <input
                value={routerSettings?.default_lang ?? "en"}
                disabled={isLoadingActions || !routerSettings}
                onChange={(e) => updateRouterSettings({ default_lang: e.target.value })}
                className="px-2 py-1 rounded text-sm"
                style={{
                  width: 72,
                  background: "var(--color-bg-primary)",
                  border: "1px solid var(--color-border)",
                  color: "var(--color-text-primary)",
                }}
              />
            </div>
          </div>

          <div
            className="card p-4 space-y-2"
            style={{ background: "var(--color-bg-secondary)" }}
          >
            <div className="flex items-center justify-between">
              <div>
                <div className="font-medium text-sm" style={{ color: "var(--color-text-primary)" }}>
                  FunctionGemma (ONNX){fgCurrent ? ` — Loaded: ${fgCurrent}` : ""}
                </div>
                <div className="text-xs mt-0.5" style={{ color: "var(--color-text-tertiary)" }}>
                  Optional local model to propose tool calls from STT commits.
                </div>
              </div>
              {fgCurrent ? (
                <button
                  className="btn-secondary text-sm"
                  disabled={isLoadingActions}
                  onClick={unloadFunctionGemma}
                >
                  Unload
                </button>
              ) : null}
            </div>

            {fgError && (
              <div className="text-xs" style={{ color: "var(--color-danger)" }}>
                {fgError}
              </div>
            )}

            <div className="space-y-2">
              {fgModels.map((m) => {
                const progress = fgDownloadProgress[m.variant];
                const file = fgDownloadFile[m.variant];
                const isLoaded = fgCurrent === m.variant;
                const isDownloading = m.is_downloading || typeof progress === "number";

                return (
                  <div
                    key={m.variant}
                    className="flex items-center justify-between gap-3 rounded-lg px-3 py-2"
                    style={{ background: "var(--color-bg-primary)", border: "1px solid var(--color-border)" }}
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
                            disabled={isLoadingActions}
                            style={{ color: "var(--color-danger)" }}
                            onClick={() => cancelFunctionGemmaDownload(m.variant)}
                          >
                            Cancel
                          </button>
                        ) : (
                          <button
                            className="btn-primary text-sm"
                            disabled={isLoadingActions}
                            onClick={() => downloadFunctionGemma(m.variant)}
                          >
                            Download
                          </button>
                        )
                      ) : isLoaded ? (
                        <button
                          className="btn-secondary text-sm"
                          disabled={isLoadingActions}
                          onClick={unloadFunctionGemma}
                        >
                          Unload
                        </button>
                      ) : (
                        <button
                          className="btn-primary text-sm"
                          disabled={isLoadingActions}
                          onClick={() => loadFunctionGemma(m.variant)}
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
        </div>
      </section>

      <section>
        <SectionHeader>Storage</SectionHeader>
        <StorageSettings />
      </section>

      <section>
        <SectionHeader>About</SectionHeader>
        <AboutSection />
      </section>
    </div>
  );
}

export { Settings as ModelSettings };
