import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useRecordingStore } from "../stores/recording-store";

export interface ModelInfo {
  name: string;
  dir_name: string;
  is_downloaded: boolean;
  size_bytes: number;
  /** Supported language codes. Empty means multilingual with auto-detect. */
  supported_languages: string[];
}

export interface TranscriptSegment {
  text: string;
  start_ms: number;
  end_ms: number;
  speaker: number | null;
}

export type Language = "auto" | "en" | "es" | "ca";

export function useStt() {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [currentModel, setCurrentModel] = useState<string | null>(null);
  const [loadingModel, setLoadingModel] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<Record<string, number>>({});
  const [language, setLanguageState] = useState<Language>("auto");
  const setStoreCurrentModel = useRecordingStore((state) => state.setCurrentModel);

  const refreshModels = useCallback(async () => {
    try {
      const modelList = await invoke<ModelInfo[]>("plugin:gibberish-stt|list_models");
      setModels(modelList);
    } catch (err) {
      console.error("Failed to list models:", err);
    }
  }, []);

  const getCurrentModel = useCallback(async () => {
    try {
      const model = await invoke<string | null>("plugin:gibberish-stt|get_current_model");
      setCurrentModel(model);
      setStoreCurrentModel(model);
      return model;
    } catch (err) {
      console.error("Failed to get current model:", err);
      return null;
    }
  }, [setStoreCurrentModel]);

  const getLanguage = useCallback(async () => {
    try {
      const lang = await invoke<string>("plugin:gibberish-stt|get_language");
      setLanguageState(lang as Language);
      return lang;
    } catch (err) {
      console.error("Failed to get language:", err);
      return "auto";
    }
  }, []);

  const setLanguage = useCallback(async (lang: Language) => {
    setLoadingModel(currentModel); // Show loading state while reloading
    try {
      await invoke("plugin:gibberish-stt|set_language", { language: lang });
      setLanguageState(lang);
      // Persist to localStorage
      localStorage.setItem("gibberish:language", lang);
    } catch (err) {
      console.error("Failed to set language:", err);
      throw err;
    } finally {
      setLoadingModel(null);
    }
  }, [currentModel]);

  // Auto-load the last used model on startup
  const autoLoadLastModel = useCallback(async (availableModels: ModelInfo[]) => {
    const lastModel = localStorage.getItem("gibberish:last-model");
    if (!lastModel) return;

    // Check if the model is downloaded
    const model = availableModels.find(m => m.name === lastModel);
    if (model?.is_downloaded) {
      console.log("Auto-loading last used model:", lastModel);
      try {
        await invoke("plugin:gibberish-stt|load_model", { modelName: lastModel });
        setCurrentModel(lastModel);
        setStoreCurrentModel(lastModel);
      } catch (err) {
        console.error("Failed to auto-load model:", err);
      }
    }
  }, [setStoreCurrentModel]);

  useEffect(() => {
    let mounted = true;

    const init = async () => {
      await refreshModels();
      await getLanguage();
      const current = await getCurrentModel();

      // If no model is currently loaded, try to auto-load the last one
      if (!current && mounted) {
        const modelList = await invoke<ModelInfo[]>("plugin:gibberish-stt|list_models");
        await autoLoadLastModel(modelList);
      }
    };

    init();

    let unlisten: (() => void) | null = null;

    listen<[string, number]>("stt:download-progress", (event) => {
      if (mounted) {
        const [modelName, progress] = event.payload;
        setDownloadProgress((prev) => ({ ...prev, [modelName]: progress }));
      }
    }).then((fn) => {
      if (mounted) {
        unlisten = fn;
      } else {
        fn();
      }
    });

    return () => {
      mounted = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [refreshModels, getCurrentModel, getLanguage, autoLoadLastModel]);

  const downloadModel = useCallback(async (modelName: string) => {
    setDownloadProgress((prev) => ({ ...prev, [modelName]: 0 }));
    try {
      const path = await invoke<string>("plugin:gibberish-stt|download_model", {
        modelName,
      });
      await refreshModels();
      return path;
    } catch (err) {
      console.error("Failed to download model:", err);
      throw err;
    } finally {
      setDownloadProgress((prev) => {
        const { [modelName]: _, ...rest } = prev;
        return rest;
      });
    }
  }, [refreshModels]);

  const cancelDownload = useCallback(async (modelName: string) => {
    try {
      await invoke("plugin:gibberish-stt|cancel_download", { modelName });
      setDownloadProgress((prev) => {
        const { [modelName]: _, ...rest } = prev;
        return rest;
      });
    } catch (err) {
      console.error("Failed to cancel download:", err);
      throw err;
    }
  }, []);

  const checkIsDownloading = useCallback(async (modelName: string) => {
    try {
      return await invoke<boolean>("plugin:gibberish-stt|is_downloading", { modelName });
    } catch (err) {
      console.error("Failed to check download status:", err);
      return false;
    }
  }, []);

  const loadModel = useCallback(async (modelName: string) => {
    setLoadingModel(modelName);
    try {
      await invoke("plugin:gibberish-stt|load_model", { modelName });
      setCurrentModel(modelName);
      setStoreCurrentModel(modelName);
      // Save as last used model for auto-load on next startup
      localStorage.setItem("gibberish:last-model", modelName);
    } catch (err) {
      console.error("Failed to load model:", err);
      throw err;
    } finally {
      setLoadingModel(null);
    }
  }, [setStoreCurrentModel]);

  const unloadModel = useCallback(async () => {
    try {
      // Stop recording if in progress
      const { isRecording, stopRecording, setIsFinalizing } = useRecordingStore.getState();
      if (isRecording) {
        try {
          await invoke("plugin:gibberish-recorder|stop_recording");
        } catch {
          // Ignore errors from stopping recording
        }
        stopRecording();
        setIsFinalizing(false);
      }

      await invoke("plugin:gibberish-stt|unload_model");
      setCurrentModel(null);
      setStoreCurrentModel(null);
    } catch (err) {
      console.error("Failed to unload model:", err);
      throw err;
    }
  }, [setStoreCurrentModel]);

  const transcribeAudio = useCallback(async (audioSamples: Float32Array) => {
    try {
      const segments = await invoke<TranscriptSegment[]>(
        "plugin:gibberish-stt|transcribe_audio",
        { audioSamples: Array.from(audioSamples) }
      );
      return segments;
    } catch (err) {
      console.error("Failed to transcribe audio:", err);
      throw err;
    }
  }, []);

  const transcribeFile = useCallback(async (filePath: string) => {
    try {
      const segments = await invoke<TranscriptSegment[]>(
        "plugin:gibberish-stt|transcribe_file",
        { filePath }
      );
      return segments;
    } catch (err) {
      console.error("Failed to transcribe file:", err);
      throw err;
    }
  }, []);

  // Get the supported languages for the current model (empty = multilingual)
  const currentModelSupportedLanguages = currentModel
    ? models.find((m) => m.name === currentModel)?.supported_languages ?? []
    : [];

  // Multilingual if supported_languages is empty (supports all languages)
  const isCurrentModelMultilingual = currentModelSupportedLanguages.length === 0;

  return {
    models,
    currentModel,
    loadingModel,
    downloadProgress,
    language,
    isCurrentModelMultilingual,
    currentModelSupportedLanguages,
    refreshModels,
    downloadModel,
    cancelDownload,
    checkIsDownloading,
    loadModel,
    unloadModel,
    setLanguage,
    transcribeAudio,
    transcribeFile,
  };
}
