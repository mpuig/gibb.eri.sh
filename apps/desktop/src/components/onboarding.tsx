import { useState, useEffect } from "react";
import { useStt } from "../hooks/use-stt";
import { useOnboardingStore } from "../stores/onboarding-store";

const RECOMMENDED_MODEL = "whisper-onnx-small";

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function Onboarding() {
  const [isDownloading, setIsDownloading] = useState(false);
  const { models, downloadProgress, downloadModel, loadModel, currentModel } = useStt();
  const { completeOnboarding } = useOnboardingStore();

  const recommendedModel = models.find((m) => m.name === RECOMMENDED_MODEL);
  const progress = downloadProgress[RECOMMENDED_MODEL] ?? 0;
  const isModelDownloaded = recommendedModel?.is_downloaded ?? false;
  const isModelLoaded = currentModel === RECOMMENDED_MODEL;

  // Auto-load model after download completes
  useEffect(() => {
    if (isModelDownloaded && isDownloading && !isModelLoaded) {
      loadModel(RECOMMENDED_MODEL).then(() => {
        completeOnboarding();
      });
    }
  }, [isModelDownloaded, isDownloading, isModelLoaded, loadModel, completeOnboarding]);

  // If model already downloaded, just load and continue
  useEffect(() => {
    if (isModelDownloaded && !isDownloading) {
      loadModel(RECOMMENDED_MODEL).then(() => {
        completeOnboarding();
      });
    }
  }, []);

  const handleGetStarted = async () => {
    if (isModelDownloaded) {
      await loadModel(RECOMMENDED_MODEL);
      completeOnboarding();
    } else {
      setIsDownloading(true);
      await downloadModel(RECOMMENDED_MODEL);
    }
  };

  return (
    <div
      className="fixed inset-0 flex flex-col items-center justify-center px-8"
      style={{ background: "var(--color-bg-primary)" }}
    >
      {/* Logo */}
      <div
        className="w-24 h-24 rounded-3xl flex items-center justify-center mb-8"
        style={{
          background: "linear-gradient(135deg, #0A84FF 0%, #5E5CE6 100%)",
        }}
      >
        <svg className="w-12 h-12" viewBox="0 0 24 24" fill="white">
          <path d="M12 14a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v5a3 3 0 0 0 3 3z" />
          <path d="M19 11a1 1 0 1 0-2 0 5 5 0 0 1-10 0 1 1 0 1 0-2 0 7 7 0 0 0 6 6.92V20H8a1 1 0 1 0 0 2h8a1 1 0 1 0 0-2h-3v-2.08A7 7 0 0 0 19 11z" />
        </svg>
      </div>

      {/* Title */}
      <h1
        className="text-3xl font-bold mb-3 text-center"
        style={{ color: "var(--color-text-primary)" }}
      >
        Gibberish
      </h1>
      <p
        className="text-center mb-8 max-w-sm"
        style={{ color: "var(--color-text-tertiary)" }}
      >
        Private speech-to-text that runs entirely on your device.
      </p>

      {/* Features */}
      <div className="space-y-3 mb-10 max-w-xs w-full">
        {[
          { icon: "🔒", text: "100% local - your audio never leaves" },
          { icon: "⚡", text: "Real-time transcription" },
          { icon: "🌍", text: "Works offline, no internet needed" },
        ].map((feature, i) => (
          <div
            key={i}
            className="flex items-center gap-3 px-4 py-3 rounded-xl"
            style={{ background: "var(--color-bg-secondary)" }}
          >
            <span className="text-lg">{feature.icon}</span>
            <span className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              {feature.text}
            </span>
          </div>
        ))}
      </div>

      {/* Action */}
      {isDownloading ? (
        <div className="w-full max-w-xs">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              Downloading model...
            </span>
            <span className="text-sm" style={{ color: "var(--color-text-tertiary)" }}>
              {progress}%
            </span>
          </div>
          <div
            className="h-2 rounded-full overflow-hidden"
            style={{ background: "var(--color-bg-tertiary)" }}
          >
            <div
              className="h-full rounded-full transition-all duration-300"
              style={{
                width: `${progress}%`,
                background: "var(--color-accent)",
              }}
            />
          </div>
          <p
            className="text-xs text-center mt-3"
            style={{ color: "var(--color-text-quaternary)" }}
          >
            {recommendedModel ? formatBytes(recommendedModel.size_bytes) : ""}
          </p>
        </div>
      ) : (
        <button
          onClick={handleGetStarted}
          className="px-8 py-3 rounded-xl font-medium text-white transition-all hover:scale-105"
          style={{ background: "var(--color-accent)" }}
        >
          Get Started
        </button>
      )}

      {/* Footer */}
      <p
        className="absolute bottom-6 text-xs"
        style={{ color: "var(--color-text-quaternary)" }}
      >
        Powered by Whisper
      </p>
    </div>
  );
}
