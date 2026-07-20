import { Pause, Play, SkipBack, Warning } from "@phosphor-icons/react";
import { useEffect, useReducer, useRef } from "react";
import { deriveWarnings, flightDuration, sampleAt } from "../core/playback";
import { createReviewClock, reduceReviewClock } from "../core/reviewClock";
import { semanticTimeline } from "../core/semanticTimeline";
import type { CounterfactualReview, FlightTrace, RunRecord } from "../core/types";
import EvidenceDrawer from "./EvidenceDrawer";

function Plot({
  record,
  field,
  label,
  unit,
  color,
  cursorT,
  proposal,
  measured,
  alignment,
}: {
  record: RunRecord;
  field: "altitude_m" | "velocity_ms";
  label: string;
  unit: string;
  color: string;
  cursorT: number;
  proposal?: RunRecord;
  measured?: FlightTrace | null;
  alignment?: Record<string, unknown> | null;
}) {
  const width = 780;
  const height = 168;
  const pad = { left: 38, right: 12, top: 18, bottom: 24 };
  const duration = Math.max(flightDuration(record), proposal ? flightDuration(proposal) : 0);
  const measuredSeries = traceSeries(measured, field, alignment);
  const values = [
    ...record.samples.map((sample) => sample[field]),
    ...(proposal?.samples.map((sample) => sample[field]) ?? []),
    ...measuredSeries.map((sample) => sample.value),
  ];
  const min = Math.min(0, ...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const x = (time: number) => pad.left + (time / duration) * (width - pad.left - pad.right);
  const y = (value: number) => height - pad.bottom - ((value - min) / span) * (height - pad.top - pad.bottom);
  const points = record.samples.map((sample) => `${x(sample.t)},${y(sample[field])}`).join(" ");
  const proposalPoints = proposal?.samples.map((sample) => `${x(sample.t)},${y(sample[field])}`).join(" ");
  const measuredPoints = measuredSeries.map((sample) => `${x(sample.time)},${y(sample.value)}`).join(" ");
  const cursor = sampleAt(record, cursorT);
  const ticks = [0, 0.25, 0.5, 0.75, 1];

  return (
    <section className="telemetry-plot">
      <header>
        <div><span>{label}</span><strong>{cursor[field].toFixed(2)} {unit}</strong></div>
        <span>t = {cursorT.toFixed(2)} s</span>
      </header>
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`${label} over flight time`}>
        {ticks.map((tick) => (
          <g key={tick}>
            <line x1={x(tick * duration)} y1={pad.top} x2={x(tick * duration)} y2={height - pad.bottom} className="plot-gridline" />
            <text x={x(tick * duration)} y={height - 7} textAnchor="middle">{(tick * duration).toFixed(0)}s</text>
          </g>
        ))}
        {[0, 0.5, 1].map((tick) => (
          <g key={`y-${tick}`}>
            <line x1={pad.left} y1={y(min + tick * span)} x2={width - pad.right} y2={y(min + tick * span)} className="plot-gridline" />
            <text x={pad.left - 6} y={y(min + tick * span) + 3} textAnchor="end">{(min + tick * span).toFixed(0)}</text>
          </g>
        ))}
        <path d={`M ${pad.left} ${y(0)} H ${width - pad.right}`} className="plot-zero" />
        <polyline points={points} fill="none" stroke={color} strokeWidth="1.7" vectorEffect="non-scaling-stroke" />
        {proposalPoints && <polyline points={proposalPoints} fill="none" stroke="var(--amber)" strokeWidth="2" strokeDasharray="6 3" vectorEffect="non-scaling-stroke" />}
        {measuredPoints && <polyline points={measuredPoints} fill="none" stroke="var(--magenta)" strokeWidth="1.5" strokeDasharray="2 3" vectorEffect="non-scaling-stroke" />}
        <line x1={x(cursorT)} y1={pad.top} x2={x(cursorT)} y2={height - pad.bottom} className="plot-cursor" />
        <circle cx={x(cursorT)} cy={y(cursor[field])} r="4" fill={color} stroke="var(--bg-canvas)" strokeWidth="2" vectorEffect="non-scaling-stroke" />
      </svg>
    </section>
  );
}

export default function FlightMode({ record, counterfactual }: { record: RunRecord; counterfactual?: CounterfactualReview | null }) {
  const duration = Math.max(flightDuration(record), counterfactual ? flightDuration(counterfactual.proposed_run) : 0);
  const [clock, dispatchClock] = useReducer(reduceReviewClock, duration, createReviewClock);
  const { time, playing } = clock;
  const animationFrame = useRef<number | undefined>(undefined);

  useEffect(() => {
    if (!playing) return;
    let last = performance.now();
    const tick = (now: number) => {
      const delta = (now - last) / 1000;
      last = now;
      dispatchClock({ type: "tick", elapsedSeconds: delta });
      animationFrame.current = requestAnimationFrame(tick);
    };
    animationFrame.current = requestAnimationFrame(tick);
    return () => {
      if (animationFrame.current) cancelAnimationFrame(animationFrame.current);
    };
  }, [playing]);

  const current = sampleAt(record, time);
  const warnings = deriveWarnings(record);
  const reviewEvents = semanticTimeline(record, counterfactual).filter((event) => event.reviewTime >= clock.range[0] && event.reviewTime <= clock.range[1]);
  const altitudeFraction = record.summary.apogee_m > 0 ? current.altitude_m / record.summary.apogee_m : 0;

  return (
    <div className="flight-workspace">
      <aside className="flight-stage">
        <div className="flight-stage-heading">
          <span className="panel-eyebrow">Playback</span>
          <h2>Flight profile</h2>
        </div>
        <div className="altitude-track">
          <div className="altitude-scale"><span>APOGEE</span><span>50%</span><span>GROUND</span></div>
          <div className="flight-vehicle" style={{ transform: `translateY(${-Math.max(0, Math.min(1, altitudeFraction)) * 100}%)` }}>
            <span />
          </div>
          <div className="altitude-current">{current.altitude_m.toFixed(1)} m</div>
        </div>
        <div className="playback-controls">
          <button type="button" onClick={() => dispatchClock({ type: "seek", time: clock.range[0] })} title="Return to active range start"><SkipBack size={14} weight="fill" /></button>
          <button
            type="button"
            className="playback-primary"
            onClick={() => {
              dispatchClock({ type: "toggle" });
            }}
          >
            {playing ? <Pause size={14} weight="fill" /> : <Play size={14} weight="fill" />}
            {playing ? "Pause" : time >= duration ? "Replay" : "Launch"}
          </button>
          <select aria-label="Playback speed" value={clock.speed} onChange={(event) => dispatchClock({ type: "setSpeed", speed: Number(event.target.value) })}>
            {[0.5, 1, 2, 4, 8, 16].map((speed) => <option key={speed} value={speed}>{speed}×</option>)}
          </select>
          <select aria-label="Camera preset" value={clock.cameraPreset} onChange={(event) => dispatchClock({ type: "setCameraPreset", preset: event.target.value })}>
            <option value="mission-overview">Mission overview</option>
            <option value="vehicle-follow">Vehicle follow</option>
            <option value="recovery-wide">Recovery wide</option>
          </select>
        </div>
      </aside>

      <section className="flight-analysis">
        <header className="analysis-header">
          <div><span className="panel-eyebrow">Deterministic simulation</span><h2>Flight Telemetry</h2></div>
          <div className="analysis-hash"><span>INPUT</span><code>{record.summary.input_hash.slice(0, 16)}</code></div>
        </header>

        <div className="flight-metrics">
          {[
            ["Apogee", record.summary.apogee_m.toFixed(1), "m"],
            ["Max velocity", record.summary.max_velocity_ms.toFixed(1), "m/s"],
            ["Rail exit", record.summary.rail_exit_velocity_ms.toFixed(1), "m/s"],
            ["Landing", Math.abs(record.summary.landing_velocity_ms).toFixed(1), "m/s"],
          ].map(([label, value, unit]) => (
            <div key={label} className="flight-metric"><span>{label}</span><strong>{value}</strong><em>{unit}</em></div>
          ))}
        </div>

        {warnings.length > 0 && (
          <div className="flight-warnings">
            <Warning size={15} weight="fill" />
            <div>{warnings.map((warning) => <span key={warning.id}>{warning.message}</span>)}</div>
          </div>
        )}

        <div className="telemetry-plots">
          {counterfactual && <div className="counterfactual-plot-legend">BASELINE <i className="baseline" /> MEASURED <i className="measured" /> PROPOSAL <i className="proposal" /></div>}
          <Plot record={record} proposal={counterfactual?.proposed_run} measured={counterfactual?.measured_trace} alignment={counterfactual?.baseline_state.alignment} field="altitude_m" label="Altitude AGL" unit="m" color="var(--blue)" cursorT={time} />
          <Plot record={record} proposal={counterfactual?.proposed_run} measured={counterfactual?.measured_trace} alignment={counterfactual?.baseline_state.alignment} field="velocity_ms" label="Vertical velocity" unit="m/s" color="var(--green)" cursorT={time} />
        </div>

        <div className="timeline-panel">
          <header><span>Semantic event lanes</span><span>{reviewEvents.length} events</span></header>
          <table>
            <thead><tr><th>Lane</th><th>Event</th><th>Time</th><th>Detector</th><th>State</th></tr></thead>
            <tbody>
              {reviewEvents.map((event) => (
                <tr
                  key={event.id}
                  className={event.reviewTime <= time ? "reached" : "pending"}
                  data-lane={event.lane}
                  onClick={() => dispatchClock({ type: "selectEvent", event })}
                >
                  <td>{event.lane}</td><td>{event.type}</td><td>{event.reviewTime.toFixed(3)} s</td><td>{event.detector.id} v{event.detector.version}</td><td>{event.reviewTime <= time ? "REACHED" : "PENDING"}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {clock.selectedEvent && (
            <div className="timeline-event-evidence">
              <strong>{clock.selectedEvent.type}</strong>
              <span>{clock.selectedEvent.detector.id} v{clock.selectedEvent.detector.version}</span>
              <span>confidence {(clock.selectedEvent.detector.confidence * 100).toFixed(0)}%</span>
              <code>{clock.selectedEvent.detector.evidenceHashes[0]?.slice(0, 16)}</code>
            </div>
          )}
        </div>

        <EvidenceDrawer design={record.design} />

        <div className="timeline-scrubber">
          <span>0.00 s</span>
          <input type="range" min={clock.range[0]} max={clock.range[1]} step={duration / 1000} value={time} onChange={(event) => dispatchClock({ type: "seek", time: Number(event.target.value) })} aria-label="Flight time" />
          <span>{duration.toFixed(2)} s</span>
        </div>
      </section>
    </div>
  );
}

function traceSeries(
  trace: FlightTrace | null | undefined,
  field: "altitude_m" | "velocity_ms",
  alignment: Record<string, unknown> | null | undefined,
): Array<{ time: number; value: number }> {
  if (!trace) return [];
  const wanted = field === "altitude_m" ? ["altitude", "position"] : ["velocity"];
  const channel = trace.channels.find((candidate) => wanted.some((term) => candidate.id.toLowerCase().includes(term)));
  if (!channel) return [];
  const scale = typeof alignment?.scale === "number" ? alignment.scale : 1;
  const offset = typeof alignment?.offset_s === "number" ? alignment.offset_s : 0;
  const overlap = Array.isArray(alignment?.overlap_source_s) ? alignment.overlap_source_s as number[] : null;
  return channel.samples.filter((sample) => sample.valid && (!overlap || (sample.time >= overlap[0] && sample.time <= overlap[1]))).map((sample) => ({
    time: sample.time * scale + offset,
    value: sample.values.length >= 3 ? sample.values[2] : sample.values[0],
  })).filter((sample) => Number.isFinite(sample.time) && Number.isFinite(sample.value));
}
