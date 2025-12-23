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

interface ActionRouterState {
  events: RouterStatusEvent[];
  lastCityResult: WikipediaCityEvent | null;
  lastCityError: WikipediaCityErrorEvent | null;

  addEvent: (event: RouterStatusEvent) => void;
  setCityResult: (event: WikipediaCityEvent) => void;
  setCityError: (event: WikipediaCityErrorEvent) => void;
  clear: () => void;
}

const MAX_EVENTS = 50;

export const useActionRouterStore = create<ActionRouterState>((set) => ({
  events: [],
  lastCityResult: null,
  lastCityError: null,

  addEvent: (event) =>
    set((state) => ({
      events: [...state.events, event].slice(-MAX_EVENTS),
    })),

  setCityResult: (event) =>
    set(() => ({
      lastCityResult: event,
      lastCityError: null,
    })),

  setCityError: (event) =>
    set(() => ({
      lastCityError: event,
    })),

  clear: () =>
    set(() => ({
      events: [],
      lastCityResult: null,
      lastCityError: null,
    })),
}));

