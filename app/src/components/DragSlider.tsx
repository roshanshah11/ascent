import { useEffect, useRef, useState } from "react";
import {
  IDLE,
  reduceDrag,
  type DragEvent,
  type DragState,
  type DragTarget,
} from "../core/dragCommit";
import type { Command } from "../core/types";

interface Props {
  label: string;
  target: DragTarget;
  min: number;
  max: number;
  step: number;
  debounceMs?: number;
  onPreview: (preview: { id: number; param: string; value: number }) => void;
  onCommit: (command: Command) => void;
}

const RELEASE_KEYS = new Set([
  "ArrowLeft",
  "ArrowRight",
  "ArrowUp",
  "ArrowDown",
  "Home",
  "End",
  "PageUp",
  "PageDown",
]);

export default function DragSlider({
  label,
  target,
  min,
  max,
  step,
  debounceMs = 120,
  onPreview,
  onCommit,
}: Props) {
  const state = useRef<DragState>(IDLE);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [value, setValue] = useState(target.startValue);

  const clearPreviewTimer = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
  };

  useEffect(() => clearPreviewTimer, []);
  useEffect(() => {
    if (!state.current.target) setValue(target.startValue);
  }, [target.startValue]);

  const advance = (event: DragEvent, immediatePreview = false) => {
    const outcome = reduceDrag(state.current, event);
    state.current = outcome.state;
    if (outcome.preview) {
      clearPreviewTimer();
      if (immediatePreview) onPreview(outcome.preview);
      else {
        timer.current = setTimeout(() => {
          onPreview(outcome.preview!);
          timer.current = null;
        }, debounceMs);
      }
    }
    if (outcome.command) onCommit(outcome.command);
    return outcome;
  };

  const begin = () => {
    if (!state.current.target) advance({ type: "start", target });
  };

  const move = (next: number) => {
    begin();
    setValue(next);
    advance({ type: "move", value: next });
  };

  const release = () => {
    clearPreviewTimer();
    advance({ type: "release" });
  };

  const cancel = () => {
    clearPreviewTimer();
    const outcome = advance({ type: "cancel" }, true);
    setValue(outcome.preview?.value ?? target.startValue);
  };

  return (
    <label className="drag-slider">
      <span>{label}</span>
      <input
        aria-label={label}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture?.(event.pointerId);
          begin();
        }}
        onPointerUp={release}
        onPointerCancel={cancel}
        onChange={(event) => move(Number(event.target.value))}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            cancel();
          } else if (RELEASE_KEYS.has(event.key)) begin();
        }}
        onKeyUp={(event) => {
          if (RELEASE_KEYS.has(event.key)) release();
        }}
      />
      <output>{(value * 1000).toFixed(1)} mm</output>
    </label>
  );
}
