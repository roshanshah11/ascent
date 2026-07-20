import type { CounterfactualReview, RunRecord } from "./types";
import type { ReviewEvent } from "./reviewClock";
import { deriveWarnings, timeline } from "./playback";

/** Project every mission-review concern onto the one review-time axis. */
export function semanticTimeline(record: RunRecord, universe?: CounterfactualReview | null): ReviewEvent[] {
  const events: ReviewEvent[] = timeline(record).map((event) => ({
    id: `phase-${event.kind}-${event.t}`,
    type: event.kind,
    reviewTime: event.t,
    lane: "flight-phase",
    detector: detector("ascent-sim.interpolated-crossing", record.summary.input_hash),
  }));

  for (const warning of deriveWarnings(record)) {
    const time = warning.id.includes("rail")
      ? record.events.find((event) => event.kind.toLowerCase().includes("rail"))?.t ?? 0
      : warning.id.includes("landing")
        ? record.summary.landing_time_s
        : record.samples.reduce((best, sample) => Math.abs(sample.velocity_ms) > Math.abs(best.velocity_ms) ? sample : best).t;
    events.push({
      id: `limit-${warning.id}`,
      type: warning.message,
      reviewTime: time,
      lane: "limit",
      detector: detector(`ascent-review.${warning.id}`, record.summary.input_hash),
    });
  }

  const measured = universe?.measured_trace;
  const alignment = universe?.baseline_state.alignment;
  const scale = typeof alignment?.scale === "number" ? alignment.scale : 1;
  const offset = typeof alignment?.offset_s === "number" ? alignment.offset_s : 0;
  const measuredSamples = measured?.channels.flatMap((channel) => channel.samples.filter((sample) => sample.valid)) ?? [];
  if (measuredSamples.length > 0) {
    const sourceStart = Math.min(...measuredSamples.map((sample) => sample.time));
    const sourceEnd = Math.max(...measuredSamples.map((sample) => sample.time));
    for (const [type, sourceTime] of [["measured evidence begins", sourceStart], ["measured evidence ends", sourceEnd]] as const) {
      events.push({
        id: `evidence-${sourceTime}`,
        type,
        reviewTime: sourceTime * scale + offset,
        lane: "evidence",
        detector: detector("ascent-review.alignment-overlap", measured?.trace_id ?? record.summary.input_hash),
      });
    }
  }

  for (const channel of universe?.proposed_reconciliation?.channels ?? []) {
    const peak = channel.residuals?.reduce((best, sample) => Math.abs(sample.value) > Math.abs(best.value) ? sample : best);
    if (peak) {
      events.push({
        id: `residual-${channel.quantity}`,
        type: `${channel.quantity} maximum residual`,
        reviewTime: peak.review_time_s,
        lane: "residual",
        detector: detector("ascent-review.phase-residual-peak", universe?.proposed_document_hash ?? record.summary.input_hash),
      });
    }
  }

  if (universe) {
    events.push({
      id: "decision-counterfactual-preview",
      type: "counterfactual proposal preview",
      reviewTime: 0,
      lane: "decision",
      detector: detector("ascent-app.external-agent-proposal", universe.proposed_document_hash),
    });
  }
  return events.sort((left, right) => left.reviewTime - right.reviewTime || left.lane.localeCompare(right.lane) || left.id.localeCompare(right.id));
}

function detector(id: string, evidenceHash: string) {
  return { id, version: "1", confidence: 1, evidenceHashes: [evidenceHash] };
}
