import { create } from "zustand";

export interface RouterStatusEvent {
  phase: string;
  ts_ms: number;
  payload: unknown;
}

export interface WikiSummaryDto {
  title: string;
  summary: string;
  url: string;
  thumbnail_url?: string | null;
  coordinates?: { lat: number; lon: number } | null;
}

export interface WikipediaCityEvent {
  city: string;
  result: WikiSummaryDto;
}

export interface WikipediaCityErrorEvent {
  city: string;
  error: string;
}

export interface NoMatchEvent {
  message: string;
  text: string;
}

interface ActionRouterState {
  events: RouterStatusEvent[];
  lastCityResult: WikipediaCityEvent | null;
  lastCityError: WikipediaCityErrorEvent | null;
  lastNoMatch: NoMatchEvent | null;

  addEvent: (event: RouterStatusEvent) => void;
  setCityResult: (event: WikipediaCityEvent) => void;
  setCityError: (event: WikipediaCityErrorEvent) => void;
  setNoMatch: (event: NoMatchEvent) => void;
  clearNoMatch: () => void;
  clear: () => void;
}

const MAX_EVENTS = 50;

export const useActionRouterStore = create<ActionRouterState>((set) => ({
  events: [],
  lastCityResult: null,
  lastCityError: null,
  lastNoMatch: null,

  addEvent: (event) =>
    set((state) => ({
      events: [...state.events, event].slice(-MAX_EVENTS),
    })),

  setCityResult: (event) =>
    set(() => ({
      lastCityResult: event,
      lastCityError: null,
      lastNoMatch: null,
    })),

  setCityError: (event) =>
    set(() => ({
      lastCityError: event,
    })),

  setNoMatch: (event) =>
    set(() => ({
      lastNoMatch: event,
    })),

  clearNoMatch: () =>
    set(() => ({
      lastNoMatch: null,
    })),

  clear: () =>
    set(() => ({
      events: [],
      lastCityResult: null,
      lastCityError: null,
      lastNoMatch: null,
    })),
}));

