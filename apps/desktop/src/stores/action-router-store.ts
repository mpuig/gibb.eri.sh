import { create } from "zustand";

export interface RouterStatusEvent {
  phase: string;
  ts_ms: number;
  payload: unknown;
}

export interface SearchResultDto {
  title: string;
  summary: string;
  url: string;
  thumbnail_url?: string | null;
}

export interface SearchResultEvent {
  query: string;
  source: string;
  result: SearchResultDto;
}

export interface SearchErrorEvent {
  query: string;
  source: string;
  error: string;
}

export interface NoMatchEvent {
  message: string;
  text: string;
}

export interface SummaryEvent {
  tool: string;
  summary: string;
  ts_ms: number;
}

interface ActionRouterState {
  events: RouterStatusEvent[];
  lastSearchResult: SearchResultEvent | null;
  lastSearchError: SearchErrorEvent | null;
  lastNoMatch: NoMatchEvent | null;
  lastSummary: SummaryEvent | null;

  addEvent: (event: RouterStatusEvent) => void;
  setSearchResult: (event: SearchResultEvent) => void;
  setSearchError: (event: SearchErrorEvent) => void;
  setNoMatch: (event: NoMatchEvent) => void;
  setSummary: (event: SummaryEvent) => void;
  clearNoMatch: () => void;
  clear: () => void;
}

const MAX_EVENTS = 50;

export const useActionRouterStore = create<ActionRouterState>((set) => ({
  events: [],
  lastSearchResult: null,
  lastSearchError: null,
  lastNoMatch: null,
  lastSummary: null,

  addEvent: (event) =>
    set((state) => ({
      events: [...state.events, event].slice(-MAX_EVENTS),
    })),

  setSearchResult: (event) =>
    set(() => ({
      lastSearchResult: event,
      lastSearchError: null,
      lastNoMatch: null,
    })),

  setSearchError: (event) =>
    set(() => ({
      lastSearchError: event,
    })),

  setNoMatch: (event) =>
    set(() => ({
      lastNoMatch: event,
    })),

  setSummary: (event) =>
    set(() => ({
      lastSummary: event,
      lastNoMatch: null,
    })),

  clearNoMatch: () =>
    set(() => ({
      lastNoMatch: null,
    })),

  clear: () =>
    set(() => ({
      events: [],
      lastSearchResult: null,
      lastSearchError: null,
      lastNoMatch: null,
      lastSummary: null,
    })),
}));

