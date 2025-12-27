import { create } from "zustand";

export interface TranscriptSegment {
  id: string;
  text: string;
  startMs: number;
  endMs: number;
  speaker?: number;
  isFinal: boolean;
}

interface RecordingState {
  isRecording: boolean;
  isListening: boolean;
  isProcessing: boolean;
  isTranscribing: boolean;
  isFinalizing: boolean;
  segments: TranscriptSegment[];
  partialText: string;
  volatileText: string;
  durationMs: number;
  bufferDurationMs: number;
  currentModel: string | null;

  startRecording: () => void;
  stopRecording: () => void;
  setIsListening: (value: boolean) => void;
  addSegment: (segment: TranscriptSegment) => void;
  updateSegment: (id: string, updates: Partial<TranscriptSegment>) => void;
  clearSegments: () => void;
  finalizeTranscript: (segments: TranscriptSegment[]) => void;
  setDuration: (ms: number) => void;
  setPartialText: (text: string) => void;
  setVolatileText: (text: string) => void;
  setBufferDuration: (ms: number) => void;
  setIsTranscribing: (value: boolean) => void;
  setIsFinalizing: (value: boolean) => void;
  setCurrentModel: (model: string | null) => void;
}

export const useRecordingStore = create<RecordingState>((set) => ({
  isRecording: false,
  isListening: false,
  isProcessing: false,
  isTranscribing: false,
  isFinalizing: false,
  segments: [],
  partialText: "",
  volatileText: "",
  durationMs: 0,
  bufferDurationMs: 0,
  currentModel: null,

  startRecording: () =>
    set({
      isRecording: true,
      isFinalizing: false,
      segments: [],
      partialText: "",
      volatileText: "",
      durationMs: 0,
      bufferDurationMs: 0,
    }),

  stopRecording: () =>
    set({
      isRecording: false,
      isProcessing: true,
      isTranscribing: false,
      isFinalizing: true,
    }),

  setIsListening: (value) => set({ isListening: value }),

  addSegment: (segment) =>
    set((state) => ({ segments: [...state.segments, segment] })),

  updateSegment: (id, updates) =>
    set((state) => ({
      segments: state.segments.map((s) =>
        s.id === id ? { ...s, ...updates } : s
      ),
    })),

  clearSegments: () =>
    set({ segments: [], isProcessing: false, partialText: "", volatileText: "", isFinalizing: false }),

  finalizeTranscript: (segments) =>
    set({
      segments,
      partialText: "",
      volatileText: "",
      isProcessing: false,
      isFinalizing: false,
    }),

  setDuration: (ms) => set({ durationMs: ms }),

  setPartialText: (text) => set({ partialText: text }),

  setVolatileText: (text) => set({ volatileText: text }),

  setBufferDuration: (ms) => set({ bufferDurationMs: ms }),

  setIsTranscribing: (value) => set({ isTranscribing: value }),

  setIsFinalizing: (value) => set({ isFinalizing: value }),

  setCurrentModel: (model) => set({ currentModel: model }),
}));
