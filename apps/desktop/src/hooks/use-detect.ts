import { useEffect, useCallback, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export interface InstalledApp {
  id: string;
  name: string;
}

export interface DetectEvent {
  type: "micStarted" | "micStopped";
  key?: string;
  apps: InstalledApp[];
}

interface UseDetectOptions {
  onMicStarted?: (apps: InstalledApp[], key: string) => void;
  onMicStopped?: (apps: InstalledApp[]) => void;
  autoRecordMeetings?: boolean;
}

export function useDetect(options: UseDetectOptions = {}) {
  const { onMicStarted, onMicStopped } = options;
  const [activeMicApps, setActiveMicApps] = useState<InstalledApp[]>([]);
  const [lastDetectionKey, setLastDetectionKey] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<DetectEvent>("detect:event", (event) => {
      const payload = event.payload;

      if (payload.type === "micStarted") {
        setActiveMicApps(payload.apps);
        setLastDetectionKey(payload.key || null);
        onMicStarted?.(payload.apps, payload.key || "");
      } else if (payload.type === "micStopped") {
        setActiveMicApps([]);
        setLastDetectionKey(null);
        onMicStopped?.(payload.apps);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [onMicStarted, onMicStopped]);

  const listInstalledApps = useCallback(async (): Promise<InstalledApp[]> => {
    try {
      return await invoke<InstalledApp[]>(
        "plugin:gibberish-detect|list_installed_applications"
      );
    } catch (err) {
      console.error("Failed to list installed apps:", err);
      return [];
    }
  }, []);

  const listMicUsingApps = useCallback(async (): Promise<InstalledApp[]> => {
    try {
      return await invoke<InstalledApp[]>(
        "plugin:gibberish-detect|list_mic_using_applications"
      );
    } catch (err) {
      console.error("Failed to list mic using apps:", err);
      return [];
    }
  }, []);

  const setIgnoredBundleIds = useCallback(
    async (bundleIds: string[]): Promise<void> => {
      try {
        await invoke("plugin:gibberish-detect|set_ignored_bundle_ids", {
          bundleIds,
        });
      } catch (err) {
        console.error("Failed to set ignored bundle ids:", err);
      }
    },
    []
  );

  const getDefaultIgnoredBundleIds = useCallback(async (): Promise<
    string[]
  > => {
    try {
      return await invoke<string[]>(
        "plugin:gibberish-detect|list_default_ignored_bundle_ids"
      );
    } catch (err) {
      console.error("Failed to get default ignored bundle ids:", err);
      return [];
    }
  }, []);

  return {
    activeMicApps,
    lastDetectionKey,
    listInstalledApps,
    listMicUsingApps,
    setIgnoredBundleIds,
    getDefaultIgnoredBundleIds,
  };
}
