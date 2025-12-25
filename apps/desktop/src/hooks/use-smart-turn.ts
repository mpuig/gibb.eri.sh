import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface TurnModelInfo {
  name: string;
  dir_name: string;
  is_downloaded: boolean;
  size_bytes: number;
}

export interface TurnSettings {
  enabled: boolean;
  threshold: number;
}

export interface TurnPredictionEvent {
  probability: number;
  threshold: number;
  is_complete: boolean;
  ts_ms: number;
}

export function useSmartTurn() {
  const [models, setModels] = useState<TurnModelInfo[]>([]);
  const [currentModel, setCurrentModel] = useState<string | null>(null);
  const [settings, setSettings] = useState<TurnSettings>({ enabled: false, threshold: 0.5 });
  const [downloadProgress, setDownloadProgress] = useState<Record<string, number>>({});
  const [lastPrediction, setLastPrediction] = useState<TurnPredictionEvent | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [modelList, current, turnSettings] = await Promise.all([
        invoke<TurnModelInfo[]>("plugin:gibberish-stt|list_turn_models"),
        invoke<string | null>("plugin:gibberish-stt|get_current_turn_model"),
        invoke<TurnSettings>("plugin:gibberish-stt|get_turn_settings"),
      ]);
      setModels(modelList);
      setCurrentModel(current);
      setSettings(turnSettings);
      return { modelList, current };
    } catch (err) {
      console.error("Failed to refresh Smart Turn state:", err);
      setError(String(err));
      return { modelList: [], current: null };
    }
  }, []);

  // Auto-load the last used Smart Turn model on startup
  const autoLoadLastModel = useCallback(async (availableModels: TurnModelInfo[]) => {
    const lastModel = localStorage.getItem("gibberish:last-turn-model");
    if (!lastModel) return;

    const model = availableModels.find(m => m.name === lastModel);
    if (model?.is_downloaded) {
      console.log("Auto-loading last used Smart Turn model:", lastModel);
      try {
        await invoke("plugin:gibberish-stt|load_turn_model", { modelName: lastModel });
        setCurrentModel(lastModel);
      } catch (err) {
        console.error("Failed to auto-load Smart Turn model:", err);
      }
    }
  }, []);

  useEffect(() => {
    let mounted = true;
    const unlisteners: (() => void)[] = [];

    const init = async () => {
      const { modelList, current } = await refresh();
      // If no model is currently loaded, try to auto-load the last one
      if (!current && mounted) {
        await autoLoadLastModel(modelList);
      }
    };
    init();

    listen<[string, number]>("stt:turn-download-progress", (event) => {
      if (!mounted) return;
      const [modelName, progress] = event.payload;
      setDownloadProgress((prev) => ({ ...prev, [modelName]: progress }));
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    listen<TurnPredictionEvent>("stt:turn_prediction", (event) => {
      if (!mounted) return;
      setLastPrediction(event.payload);
    }).then((un) => {
      if (mounted) unlisteners.push(un);
      else un();
    });

    return () => {
      mounted = false;
      unlisteners.forEach((u) => u());
    };
  }, [refresh, autoLoadLastModel]);

  const downloadModel = useCallback(
    async (modelName: string) => {
      setIsLoading(true);
      setError(null);
      setDownloadProgress((prev) => ({ ...prev, [modelName]: 0 }));
      try {
        await invoke<string>("plugin:gibberish-stt|download_turn_model", { modelName });
        await refresh();
      } catch (err) {
        console.error("Failed to download turn model:", err);
        setError(String(err));
      } finally {
        setIsLoading(false);
      }
    },
    [refresh]
  );

  const cancelDownload = useCallback(async (modelName: string) => {
    setIsLoading(true);
    try {
      await invoke("plugin:gibberish-stt|cancel_turn_download", { modelName });
      setDownloadProgress((prev) => {
        const { [modelName]: _, ...rest } = prev;
        return rest;
      });
      await refresh();
    } catch (err) {
      console.error("Failed to cancel turn model download:", err);
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, [refresh]);

  const loadModel = useCallback(
    async (modelName: string) => {
      setIsLoading(true);
      setError(null);
      try {
        await invoke("plugin:gibberish-stt|load_turn_model", { modelName });
        setCurrentModel(modelName);
        // Save as last used model for auto-load on next startup
        localStorage.setItem("gibberish:last-turn-model", modelName);
        await refresh();
      } catch (err) {
        console.error("Failed to load turn model:", err);
        setError(String(err));
      } finally {
        setIsLoading(false);
      }
    },
    [refresh]
  );

  const unloadModel = useCallback(async () => {
    setIsLoading(true);
    try {
      await invoke("plugin:gibberish-stt|unload_turn_model");
      await refresh();
    } catch (err) {
      console.error("Failed to unload turn model:", err);
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, [refresh]);

  const updateSettings = useCallback(
    async (enabled: boolean, threshold: number) => {
      setIsLoading(true);
      setError(null);
      try {
        const updated = await invoke<TurnSettings>("plugin:gibberish-stt|set_turn_settings", {
          enabled,
          threshold,
        });
        setSettings(updated);
      } catch (err) {
        console.error("Failed to update turn settings:", err);
        setError(String(err));
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  return {
    models,
    currentModel,
    settings,
    downloadProgress,
    lastPrediction,
    isLoading,
    error,
    refresh,
    downloadModel,
    cancelDownload,
    loadModel,
    unloadModel,
    updateSettings,
  };
}

