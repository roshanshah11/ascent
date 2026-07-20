export interface ReviewEventDetector {
  id: string;
  version: string;
  confidence: number;
  evidenceHashes: string[];
}

export interface ReviewEvent {
  id: string;
  type: string;
  reviewTime: number;
  lane: "flight-phase" | "limit" | "evidence" | "residual" | "decision";
  detector: ReviewEventDetector;
}

export interface ClockTransform {
  offset: number;
  scale: number;
}

export interface ReviewClockState {
  duration: number;
  time: number;
  playing: boolean;
  speed: number;
  range: readonly [number, number];
  selectedEvent: ReviewEvent | null;
  cameraPreset: string;
}

export type ReviewClockAction =
  | { type: "play" }
  | { type: "pause" }
  | { type: "toggle" }
  | { type: "seek"; time: number }
  | { type: "step"; seconds: number }
  | { type: "tick"; elapsedSeconds: number }
  | { type: "setSpeed"; speed: number }
  | { type: "setRange"; range: readonly [number, number] }
  | { type: "selectEvent"; event: ReviewEvent }
  | { type: "setCameraPreset"; preset: string };

export function createReviewClock(duration: number): ReviewClockState {
  if (!Number.isFinite(duration) || duration < 0) throw new Error("review duration must be finite and non-negative");
  return {
    duration,
    time: 0,
    playing: false,
    speed: 1,
    range: [0, duration],
    selectedEvent: null,
    cameraPreset: "mission-overview",
  };
}

export function reduceReviewClock(state: ReviewClockState, action: ReviewClockAction): ReviewClockState {
  switch (action.type) {
    case "play":
      return { ...state, playing: state.time < state.range[1] };
    case "pause":
      return { ...state, playing: false };
    case "toggle":
      return state.playing
        ? { ...state, playing: false }
        : { ...state, time: state.time >= state.range[1] ? state.range[0] : state.time, playing: true };
    case "seek":
      return { ...state, time: clampFinite(action.time, state.range), playing: false, selectedEvent: null };
    case "step":
      return {
        ...state,
        time: clampFinite(state.time + finite(action.seconds, "step"), state.range),
        playing: false,
        selectedEvent: null,
      };
    case "tick": {
      if (!state.playing) return state;
      const next = state.time + finite(action.elapsedSeconds, "elapsed time") * state.speed;
      if (next >= state.range[1]) return { ...state, time: state.range[1], playing: false };
      return { ...state, time: Math.max(state.range[0], next) };
    }
    case "setSpeed": {
      const speed = finite(action.speed, "playback speed");
      if (speed <= 0 || speed > 64) throw new Error("playback speed must be in (0, 64]");
      return { ...state, speed };
    }
    case "setRange": {
      const start = finite(action.range[0], "range start");
      const end = finite(action.range[1], "range end");
      if (start < 0 || end > state.duration || start > end) throw new Error("review range is outside duration or reversed");
      const range = [start, end] as const;
      return { ...state, range, time: clampFinite(state.time, range), playing: false };
    }
    case "selectEvent": {
      const eventTime = finite(action.event.reviewTime, "event time");
      if (eventTime < state.range[0] || eventTime > state.range[1]) throw new Error("event is outside active review range");
      if (!Number.isFinite(action.event.detector.confidence) || action.event.detector.confidence < 0 || action.event.detector.confidence > 1) {
        throw new Error("event detector confidence must be in [0, 1]");
      }
      return { ...state, time: eventTime, playing: false, selectedEvent: action.event };
    }
    case "setCameraPreset":
      if (!action.preset.trim()) throw new Error("camera preset must be named");
      return { ...state, cameraPreset: action.preset };
  }
}

export function resolveDomainTime(sourceTime: number, transform: ClockTransform): number {
  const scale = finite(transform.scale, "clock scale");
  const offset = finite(transform.offset, "clock offset");
  if (scale <= 0) throw new Error("clock scale must be positive");
  return finite(sourceTime, "source time") * scale + offset;
}

function finite(value: number, label: string): number {
  if (!Number.isFinite(value)) throw new Error(`${label} must be finite`);
  return value;
}

function clampFinite(value: number, range: readonly [number, number]): number {
  return Math.min(range[1], Math.max(range[0], finite(value, "review time")));
}
