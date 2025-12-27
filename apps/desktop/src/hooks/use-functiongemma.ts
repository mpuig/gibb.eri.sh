import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface FunctionGemmaModelInfo {
  variant: string;
  is_downloaded: boolean;
  size_bytes: number;
  is_downloading: boolean;
}

interface DownloadProgressEvent {
  variant: string;
  file: string;
  progress: number;
  downloaded_bytes: number;
  total_bytes: number;
  file_downloaded_bytes: number;
  file_total_bytes: number;
}

export function useFunctionGemma() {
  const [models, setModels] = useState<FunctionGemmaModelInfo[]>([]);
  const [currentModel, setCurrentModel] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<Record<string, number>>({});
  const [downloadFile, setDownloadFile] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const refresh = useCallback(async () => {
    const [modelList, current] = await Promise.all([
      invoke<FunctionGemmaModelInfo[]>("plugin:gibberish-tools|list_functiongemma_models"),
      invoke<string | null>("plugin:gibberish-tools|get_current_functiongemma_model"),
    ]);
    setModels(modelList);
    setCurrentModel(current);
    return { modelList, current };
  }, []);

  // Auto-load the last used FunctionGemma model on startup
  const autoLoadLastModel = useCallback(async (availableModels: FunctionGemmaModelInfo[]) => {
    const lastModel = localStorage.getItem("gibberish:last-functiongemma-model");
    if (!lastModel) return;

    const model = availableModels.find(m => m.variant === lastModel);
    if (model?.is_downloaded) {
      console.log("Auto-loading last used FunctionGemma model:", lastModel);
      try {
        await invoke("plugin:gibberish-tools|load_functiongemma_model", { variant: lastModel });
        setCurrentModel(lastModel);
      } catch (err) {
        console.error("Failed to auto-load FunctionGemma model:", err);
      }
    }
  }, []);

  useEffect(() => {
    let mounted = true;
    const unlisteners: (() => void)[] = [];

    const init = async () => {
      try {
        const { modelList, current } = await refresh();
        // If no model is currently loaded, try to auto-load the last one
        if (!current && mounted) {
          await autoLoadLastModel(modelList);
        }
      } catch (err) {
        console.error("Failed to initialize FunctionGemma:", err);
        if (mounted) {
          setError(String(err));
        }
      }
    };
    init();

    listen<DownloadProgressEvent>("tools:functiongemma_download_progress", (event) => {
      if (!mounted) return;
      setDownloadProgress((prev) => ({ ...prev, [event.payload.variant]: event.payload.progress }));
      setDownloadFile((prev) => ({ ...prev, [event.payload.variant]: event.payload.file }));
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    listen<{ variant: string }>("tools:functiongemma_download_complete", async () => {
      if (!mounted) return;
      await refresh();
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    listen<{ variant: string; error: string }>("tools:functiongemma_download_error", async (event) => {
      if (!mounted) return;
      setError(event.payload.error);
      await refresh();
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    listen<{ variant: string }>("tools:functiongemma_loaded", async () => {
      if (!mounted) return;
      await refresh();
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    return () => {
      mounted = false;
      unlisteners.forEach((u) => u());
    };
  }, [refresh, autoLoadLastModel]);

  const downloadModel = useCallback(async (variant: string) => {
    setIsLoading(true);
    setError(null);
    setDownloadProgress((prev) => ({ ...prev, [variant]: 0 }));
    try {
      const p = invoke<string>("plugin:gibberish-tools|download_functiongemma_model", { variant });
      setModels((prev) =>
        prev.map((m) => (m.variant === variant ? { ...m, is_downloading: true } : m))
      );
      await p;
      await refresh();
    } catch (err) {
      console.error("Failed to download FunctionGemma:", err);
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, [refresh]);

  const cancelDownload = useCallback(async (variant: string) => {
    setIsLoading(true);
    try {
      await invoke<boolean>("plugin:gibberish-tools|cancel_functiongemma_download", { variant });
      await refresh();
      setDownloadProgress((prev) => {
        const { [variant]: _, ...rest } = prev;
        return rest;
      });
      setDownloadFile((prev) => {
        const { [variant]: _, ...rest } = prev;
        return rest;
      });
    } catch (err) {
      console.error("Failed to cancel FunctionGemma download:", err);
    } finally {
      setIsLoading(false);
    }
  }, [refresh]);

  const loadModel = useCallback(async (variant: string) => {
    setIsLoading(true);
    setError(null);
    try {
      await invoke("plugin:gibberish-tools|load_functiongemma_model", { variant });
      setCurrentModel(variant);
      // Save as last used model for auto-load on next startup
      localStorage.setItem("gibberish:last-functiongemma-model", variant);
      await refresh();
    } catch (err) {
      console.error("Failed to load FunctionGemma:", err);
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, [refresh]);

  const unloadModel = useCallback(async () => {
    setIsLoading(true);
    try {
      await invoke("plugin:gibberish-tools|unload_functiongemma_model");
      await refresh();
    } catch (err) {
      console.error("Failed to unload FunctionGemma:", err);
    } finally {
      setIsLoading(false);
    }
  }, [refresh]);

  return {
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
    clearError: () => setError(null),
  };
}
