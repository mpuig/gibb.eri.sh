import { useState, useCallback } from "react";
import { useRecordingStore } from "../stores/recording-store";
import { useDetect, InstalledApp } from "./use-detect";
import { useRecording } from "./use-recording";

const MEETING_BUNDLE_IDS = [
  "us.zoom.xos",
  "Cisco-Systems.Spark",
  "com.microsoft.teams",
  "com.microsoft.teams2",
  "com.discord.Discord",
  "com.slack.Slack",
];

function isMeetingApp(app: InstalledApp): boolean {
  return (
    MEETING_BUNDLE_IDS.includes(app.id) ||
    app.name.toLowerCase().includes("zoom") ||
    app.name.toLowerCase().includes("teams") ||
    app.name.toLowerCase().includes("meet") ||
    app.name.toLowerCase().includes("webex")
  );
}

export function useAutoRecord() {
  const { isRecording, currentModel } = useRecordingStore();
  const { startRecording, stopRecording } = useRecording();
  const [notification, setNotification] = useState<string | null>(null);
  const [autoRecordedMeetingApp, setAutoRecordedMeetingApp] = useState<string | null>(null);

  const handleMicStarted = useCallback(
    (apps: InstalledApp[], _key: string) => {
      if (isRecording || !currentModel) return;

      const meetingApps = apps.filter(isMeetingApp);

      if (meetingApps.length > 0) {
        const appNames = meetingApps.map((a) => a.name).join(", ");
        const firstMeetingAppId = meetingApps[0].id;
        console.log("Meeting detected, will auto-start recording in 2s:", appNames);
        setNotification(`Auto-recording: ${appNames}`);
        setAutoRecordedMeetingApp(firstMeetingAppId);

        setTimeout(() => {
          if (!useRecordingStore.getState().isRecording) {
            startRecording();
          }
        }, 2000);

        setTimeout(() => setNotification(null), 5000);
      }
    },
    [isRecording, currentModel, startRecording]
  );

  const handleMicStopped = useCallback(
    (apps: InstalledApp[]) => {
      if (!autoRecordedMeetingApp) return;

      const stoppedMeetingApp = apps.find((app) => app.id === autoRecordedMeetingApp);

      if (stoppedMeetingApp) {
        console.log("Meeting ended, auto-stopping recording:", stoppedMeetingApp.name);
        setNotification(`Meeting ended: ${stoppedMeetingApp.name}`);
        setAutoRecordedMeetingApp(null);

        if (useRecordingStore.getState().isRecording) {
          stopRecording();
        }

        setTimeout(() => setNotification(null), 5000);
      }
    },
    [autoRecordedMeetingApp, stopRecording]
  );

  useDetect({
    onMicStarted: handleMicStarted,
    onMicStopped: handleMicStopped,
  });

  return {
    notification,
    autoRecordedMeetingApp,
  };
}
