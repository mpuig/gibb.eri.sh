import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SectionHeader } from "./shared";

type ActionRouterSettings = {
  enabled: boolean;
  auto_run_read_only: boolean;
  default_lang: string;
  tool_manifest: string;
  functiongemma_instructions: string;
  min_confidence: number;
};

function ActionRouterCard() {
  const [settings, setSettings] = useState<ActionRouterSettings | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [manifestDraft, setManifestDraft] = useState("");
  const [manifestError, setManifestError] = useState<string | null>(null);
  const [langDraft, setLangDraft] = useState("en");
  const [confidenceDraft, setConfidenceDraft] = useState(0.35);
  const [instructionsDraft, setInstructionsDraft] = useState("");
  const [instructionsError, setInstructionsError] = useState<string | null>(null);

  useEffect(() => {
    invoke<ActionRouterSettings>("plugin:gibberish-tools|get_action_router_settings")
      .then((s) => {
        setSettings(s);
        setManifestDraft(s.tool_manifest ?? "");
        setLangDraft(s.default_lang ?? "en");
        setConfidenceDraft(typeof s.min_confidence === "number" ? s.min_confidence : 0.35);
        setInstructionsDraft(s.functiongemma_instructions ?? "");
      })
      .catch((err) => console.error("Failed to load action router settings:", err));
  }, []);

  const updateSettings = useCallback(async (updates: Partial<ActionRouterSettings>) => {
    if (!settings) return;
    setIsLoading(true);
    setManifestError(null);
    setInstructionsError(null);
    try {
      const next = await invoke<ActionRouterSettings>(
        "plugin:gibberish-tools|set_action_router_settings",
        {
          enabled: updates.enabled,
          autoRunReadOnly: updates.auto_run_read_only,
          defaultLang: updates.default_lang,
          toolManifest: updates.tool_manifest,
          functiongemmaInstructions: updates.functiongemma_instructions,
          minConfidence: updates.min_confidence,
        }
      );
      setSettings(next);
      if (typeof updates.tool_manifest === "string") {
        setManifestDraft(next.tool_manifest ?? "");
      }
      if (typeof updates.functiongemma_instructions === "string") {
        setInstructionsDraft(next.functiongemma_instructions ?? "");
      }
    } catch (err) {
      if (typeof updates.tool_manifest === "string") {
        setManifestError(String(err));
      }
      if (typeof updates.functiongemma_instructions === "string") {
        setInstructionsError(String(err));
      }
      console.error("Failed to update action router settings:", err);
    } finally {
      setIsLoading(false);
    }
  }, [settings]);

  // Debounce "chatty" settings so we don't spam invoke() on each keystroke/spinner tick.
  useEffect(() => {
    if (!settings || isLoading) return;
    if (langDraft === (settings.default_lang ?? "en")) return;
    const t = setTimeout(() => updateSettings({ default_lang: langDraft }), 400);
    return () => clearTimeout(t);
  }, [settings, isLoading, langDraft, updateSettings]);

  useEffect(() => {
    if (!settings || isLoading) return;
    if (confidenceDraft === (settings.min_confidence ?? 0.35)) return;
    const t = setTimeout(() => updateSettings({ min_confidence: confidenceDraft }), 400);
    return () => clearTimeout(t);
  }, [settings, isLoading, confidenceDraft, updateSettings]);

  return (
    <div className="card p-4 space-y-3" style={{ background: "var(--color-bg-secondary)" }}>
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
          disabled={isLoading || !settings}
          onClick={() => updateSettings({ enabled: !(settings?.enabled ?? true) })}
        >
          {settings?.enabled ? "Disable" : "Enable"}
        </button>
      </div>

      <div className="flex items-center justify-between">
        <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
          Auto-run read-only actions
        </div>
        <button
          className="btn-secondary text-sm"
          disabled={isLoading || !settings}
          onClick={() =>
            updateSettings({
              auto_run_read_only: !(settings?.auto_run_read_only ?? true),
            })
          }
        >
          {settings?.auto_run_read_only ? "On" : "Off"}
        </button>
      </div>

      <div className="flex items-center justify-between gap-3">
        <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
          Default Wikipedia language
        </div>
        <input
          value={langDraft}
          disabled={isLoading || !settings}
          onChange={(e) => setLangDraft(e.target.value)}
          className="px-2 py-1 rounded text-sm"
          style={{
            width: 72,
            background: "var(--color-bg-primary)",
            border: "1px solid var(--color-border)",
            color: "var(--color-text-primary)",
          }}
        />
      </div>

      <div className="flex items-center justify-between gap-3">
        <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
          Min confidence
        </div>
        <input
          type="number"
          min={0}
          max={1}
          step={0.05}
          value={confidenceDraft}
          disabled={isLoading || !settings}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (!Number.isFinite(v)) return;
            setConfidenceDraft(v);
          }}
          className="px-2 py-1 rounded text-sm"
          style={{
            width: 72,
            background: "var(--color-bg-primary)",
            border: "1px solid var(--color-border)",
            color: "var(--color-text-primary)",
          }}
        />
      </div>

      <div className="space-y-2">
        <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
          Tool manifest (JSON)
        </div>
        <textarea
          value={manifestDraft}
          disabled={isLoading || !settings}
          onChange={(e) => setManifestDraft(e.target.value)}
          className="px-2 py-1 rounded text-xs"
          rows={8}
          style={{
            width: "100%",
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace",
            background: "var(--color-bg-primary)",
            border: "1px solid var(--color-border)",
            color: "var(--color-text-primary)",
          }}
        />
        {manifestError && (
          <div className="text-xs" style={{ color: "var(--color-danger)" }}>
            {manifestError}
          </div>
        )}
        <div className="flex items-center justify-end gap-2">
          <button
            className="btn-secondary text-sm"
            disabled={isLoading || !settings}
            onClick={() => {
              if (!settings) return;
              setManifestDraft(settings.tool_manifest ?? "");
              setManifestError(null);
            }}
          >
            Revert
          </button>
          <button
            className="btn-primary text-sm"
            disabled={isLoading || !settings}
            onClick={() => updateSettings({ tool_manifest: manifestDraft })}
          >
            Save
          </button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
          FunctionGemma instructions
        </div>
        <textarea
          value={instructionsDraft}
          disabled={isLoading || !settings}
          onChange={(e) => setInstructionsDraft(e.target.value)}
          className="px-2 py-1 rounded text-xs"
          rows={5}
          style={{
            width: "100%",
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace",
            background: "var(--color-bg-primary)",
            border: "1px solid var(--color-border)",
            color: "var(--color-text-primary)",
          }}
        />
        {instructionsError && (
          <div className="text-xs" style={{ color: "var(--color-danger)" }}>
            {instructionsError}
          </div>
        )}
        <div className="flex items-center justify-end gap-2">
          <button
            className="btn-secondary text-sm"
            disabled={isLoading || !settings}
            onClick={() => {
              if (!settings) return;
              setInstructionsDraft(settings.functiongemma_instructions ?? "");
              setInstructionsError(null);
            }}
          >
            Revert
          </button>
          <button
            className="btn-primary text-sm"
            disabled={isLoading || !settings}
            onClick={() => updateSettings({ functiongemma_instructions: instructionsDraft })}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}

export function ActionsSection() {
  return (
    <section>
      <SectionHeader>Action Router</SectionHeader>
      <ActionRouterCard />
    </section>
  );
}
