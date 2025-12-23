import { useState } from "react";
import { useStt, type ModelInfo } from "../hooks/use-stt";
import { useOnboardingStore } from "../stores/onboarding-store";

type Step = "welcome" | "model" | "ready";

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function WelcomeStep({ onNext }: { onNext: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center px-8">
      <div className="w-20 h-20 bg-gradient-to-br from-blue-500 to-purple-600 rounded-2xl flex items-center justify-center mb-6">
        <span className="text-4xl font-bold text-white">.sh</span>
      </div>
      <h1 className="text-3xl font-bold text-white mb-4">Welcome to gibb.eri.sh</h1>
      <p className="text-gray-400 max-w-md mb-8">
        Private speech-to-text transcription that runs entirely on your device.
        Your audio never leaves your computer.
      </p>
      <div className="space-y-3 text-left text-gray-400 mb-8">
        <div className="flex items-start gap-3">
          <svg className="w-5 h-5 text-green-500 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          </svg>
          <span>100% local processing - no cloud required</span>
        </div>
        <div className="flex items-start gap-3">
          <svg className="w-5 h-5 text-green-500 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          </svg>
          <span>Real-time transcription as you speak</span>
        </div>
        <div className="flex items-start gap-3">
          <svg className="w-5 h-5 text-green-500 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          </svg>
          <span>Export transcripts to multiple formats</span>
        </div>
      </div>
      <button
        onClick={onNext}
        className="px-6 py-3 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-medium transition-colors"
      >
        Get Started
      </button>
    </div>
  );
}

function ModelStep({ onNext, onBack }: { onNext: () => void; onBack: () => void }) {
  const { models, downloadProgress, downloadModel, cancelDownload, loadModel, currentModel } = useStt();

  const hasDownloadedModel = models.some((m) => m.is_downloaded);
  const hasLoadedModel = currentModel !== null;

  const handleDownloadAndLoad = async (model: ModelInfo) => {
    if (!model.is_downloaded) {
      await downloadModel(model.name);
    }
  };

  const handleLoad = async (modelName: string) => {
    await loadModel(modelName);
  };

  return (
    <div className="flex flex-col h-full px-8 py-6">
      <button
        onClick={onBack}
        className="flex items-center gap-1 text-gray-400 hover:text-white mb-4 self-start"
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
        </svg>
        Back
      </button>

      <h2 className="text-2xl font-bold text-white mb-2">Download a Model</h2>
      <p className="text-gray-400 mb-6">
        Choose a speech recognition model. Smaller models are faster but less accurate.
      </p>

      <div className="flex-1 overflow-auto space-y-3">
        {models.map((model) => {
          const isDownloading = model.name in downloadProgress;
          const progress = downloadProgress[model.name] ?? 0;
          const isActive = currentModel === model.name;

          return (
            <div
              key={model.name}
              className={`p-4 rounded-lg border ${
                isActive ? "border-green-500 bg-green-900/20" : "border-gray-700 bg-gray-800"
              }`}
            >
              <div className="flex justify-between items-start">
                <div>
                  <h3 className="font-medium text-white">{model.name}</h3>
                  <p className="text-sm text-gray-400">{formatBytes(model.size_bytes)}</p>
                </div>
                <div className="flex items-center gap-2">
                  {model.is_downloaded ? (
                    isActive ? (
                      <span className="px-3 py-1 text-sm text-green-400 flex items-center gap-1">
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                        </svg>
                        Ready
                      </span>
                    ) : (
                      <button
                        onClick={() => handleLoad(model.name)}
                        className="px-3 py-1 text-sm bg-blue-600 hover:bg-blue-500 text-white rounded"
                      >
                        Load
                      </button>
                    )
                  ) : isDownloading ? (
                    <div className="flex items-center gap-2">
                      <div className="w-20 bg-gray-700 rounded-full h-2">
                        <div
                          className="bg-blue-500 h-2 rounded-full transition-all"
                          style={{ width: `${progress}%` }}
                        />
                      </div>
                      <span className="text-sm text-gray-400">{progress}%</span>
                      <button
                        onClick={() => cancelDownload(model.name)}
                        className="px-2 py-1 text-xs bg-red-600 hover:bg-red-500 text-white rounded"
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <button
                      onClick={() => handleDownloadAndLoad(model)}
                      className="px-3 py-1 text-sm bg-green-600 hover:bg-green-500 text-white rounded"
                    >
                      Download
                    </button>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      <div className="mt-6 pt-4 border-t border-gray-800">
        <button
          onClick={onNext}
          disabled={!hasLoadedModel}
          className="w-full px-4 py-3 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-medium disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {hasLoadedModel ? "Continue" : hasDownloadedModel ? "Load a model to continue" : "Download a model to continue"}
        </button>
      </div>
    </div>
  );
}

function ReadyStep({ onComplete }: { onComplete: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center px-8">
      <div className="w-16 h-16 bg-green-600 rounded-full flex items-center justify-center mb-6">
        <svg className="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
        </svg>
      </div>
      <h2 className="text-2xl font-bold text-white mb-4">You're All Set!</h2>
      <p className="text-gray-400 max-w-md mb-8">
        Click the record button to start transcribing. Your recordings will be saved automatically
        and you can find them in the Sessions tab.
      </p>
      <div className="space-y-4 text-left text-gray-400 mb-8 max-w-sm">
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 bg-red-500 rounded-full flex items-center justify-center flex-shrink-0">
            <svg className="w-4 h-4 text-white" fill="currentColor" viewBox="0 0 24 24">
              <circle cx="12" cy="12" r="6" />
            </svg>
          </div>
          <div>
            <div className="text-white font-medium">Record</div>
            <div className="text-sm">Click to start/stop recording</div>
          </div>
        </div>
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 bg-gray-700 rounded-full flex items-center justify-center flex-shrink-0">
            <svg className="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h7" />
            </svg>
          </div>
          <div>
            <div className="text-white font-medium">Sessions</div>
            <div className="text-sm">Browse past recordings</div>
          </div>
        </div>
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 bg-gray-700 rounded-full flex items-center justify-center flex-shrink-0">
            <svg className="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 10v6m0 0l-3-3m3 3l3-3m2 8H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
          </div>
          <div>
            <div className="text-white font-medium">Export</div>
            <div className="text-sm">Save as text, SRT, or JSON</div>
          </div>
        </div>
      </div>
      <button
        onClick={onComplete}
        className="px-8 py-3 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-medium transition-colors"
      >
        Start Using Gibb.eri.sh
      </button>
    </div>
  );
}

export function Onboarding() {
  const [step, setStep] = useState<Step>("welcome");
  const { completeOnboarding } = useOnboardingStore();

  const handleComplete = () => {
    completeOnboarding();
  };

  return (
    <div className="fixed inset-0 bg-gray-900 z-50">
      {step === "welcome" && <WelcomeStep onNext={() => setStep("model")} />}
      {step === "model" && (
        <ModelStep onNext={() => setStep("ready")} onBack={() => setStep("welcome")} />
      )}
      {step === "ready" && <ReadyStep onComplete={handleComplete} />}
    </div>
  );
}
