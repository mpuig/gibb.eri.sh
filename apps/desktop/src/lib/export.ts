import type { TranscriptSegment } from "../stores/recording-store";

export type ExportFormat = "markdown" | "json" | "srt";

function formatTimestamp(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${minutes}:${secs.toString().padStart(2, "0")}`;
}

function formatSrtTimestamp(ms: number): string {
  const hours = Math.floor(ms / 3600000);
  const minutes = Math.floor((ms % 3600000) / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")},${millis.toString().padStart(3, "0")}`;
}

export function exportToMarkdown(segments: TranscriptSegment[]): string {
  const lines: string[] = ["# Transcript", ""];

  for (const segment of segments) {
    const time = `${formatTimestamp(segment.startMs)} - ${formatTimestamp(segment.endMs)}`;
    const speaker = segment.speaker !== undefined ? ` (Speaker ${segment.speaker + 1})` : "";
    lines.push(`## ${time}${speaker}`);
    lines.push("");
    lines.push(segment.text);
    lines.push("");
  }

  return lines.join("\n");
}

export function exportToJson(segments: TranscriptSegment[]): string {
  const data = {
    segments: segments.map((s) => ({
      text: s.text,
      startMs: s.startMs,
      endMs: s.endMs,
      speaker: s.speaker ?? null,
    })),
    exportedAt: new Date().toISOString(),
  };
  return JSON.stringify(data, null, 2);
}

export function exportToSrt(segments: TranscriptSegment[]): string {
  const lines: string[] = [];

  segments.forEach((segment, index) => {
    lines.push((index + 1).toString());
    lines.push(`${formatSrtTimestamp(segment.startMs)} --> ${formatSrtTimestamp(segment.endMs)}`);
    lines.push(segment.text);
    lines.push("");
  });

  return lines.join("\n");
}

export function exportTranscript(segments: TranscriptSegment[], format: ExportFormat): string {
  switch (format) {
    case "markdown":
      return exportToMarkdown(segments);
    case "json":
      return exportToJson(segments);
    case "srt":
      return exportToSrt(segments);
  }
}

export function getFileExtension(format: ExportFormat): string {
  switch (format) {
    case "markdown":
      return "md";
    case "json":
      return "json";
    case "srt":
      return "srt";
  }
}

export function getMimeType(format: ExportFormat): string {
  switch (format) {
    case "markdown":
      return "text/markdown";
    case "json":
      return "application/json";
    case "srt":
      return "text/plain";
  }
}
