import { SpeechModelsSection } from "./speech-models-section";
import { SmartTurnSection } from "./smart-turn-section";
import { FunctionGemmaSection } from "./functiongemma-section";
import { ActionsSection } from "./actions-section";
import { StorageSection } from "./storage-section";
import { AboutSection } from "./about-section";

export function Settings() {
  return (
    <div className="p-6 max-w-2xl mx-auto space-y-8">
      <SpeechModelsSection />
      <SmartTurnSection />
      <FunctionGemmaSection />
      <ActionsSection />
      <StorageSection />
      <AboutSection />
    </div>
  );
}

export { Settings as ModelSettings };
