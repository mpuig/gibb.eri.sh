import { useState, useRef, useEffect } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import type { TranscriptSegment } from "../stores/recording-store";
import { exportTranscript, getFileExtension, type ExportFormat } from "../lib/export";

interface ExportMenuProps {
  segments: TranscriptSegment[];
}

const FORMATS: { value: ExportFormat; label: string; description: string }[] = [
  { value: "markdown", label: "Markdown", description: ".md" },
  { value: "json", label: "JSON", description: ".json" },
];

export function ExportMenu({ segments }: ExportMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const handleExport = async (format: ExportFormat) => {
    setIsExporting(true);
    setIsOpen(false);

    try {
      const content = exportTranscript(segments, format);
      const ext = getFileExtension(format);
      const defaultName = `transcript-${Date.now()}.${ext}`;

      const filePath = await save({
        defaultPath: defaultName,
        filters: [{ name: format.toUpperCase(), extensions: [ext] }],
      });

      if (filePath) {
        await writeTextFile(filePath, content);
      }
    } catch (err) {
      console.error("Export failed:", err);
    } finally {
      setIsExporting(false);
    }
  };

  if (segments.length === 0) {
    return null;
  }

  return (
    <div className="relative" ref={menuRef}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        disabled={isExporting}
        className="px-3 py-1 text-sm rounded bg-gray-700 text-white hover:bg-gray-600 disabled:opacity-50 flex items-center gap-1"
      >
        {isExporting ? (
          "Exporting..."
        ) : (
          <>
            Export
            <svg
              className={`w-4 h-4 transition-transform ${isOpen ? "rotate-180" : ""}`}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </>
        )}
      </button>

      {isOpen && (
        <div className="absolute right-0 bottom-full mb-1 w-48 bg-gray-800 rounded-lg shadow-lg border border-gray-700 py-1 z-10">
          {FORMATS.map((format) => (
            <button
              key={format.value}
              onClick={() => handleExport(format.value)}
              className="w-full px-4 py-2 text-left text-sm text-gray-200 hover:bg-gray-700 flex justify-between items-center"
            >
              <span>{format.label}</span>
              <span className="text-gray-500">{format.description}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
