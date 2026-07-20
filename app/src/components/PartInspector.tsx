import type { Command, PartPreview, Vehicle, VehiclePart } from "../core/types";
import DragSlider from "./DragSlider";

interface Props {
  vehicle: Vehicle;
  preview: PartPreview | null;
  previewing: boolean;
  error: string | null;
  onPreview: (candidate: { id: number; param: string; value: number }) => void;
  onCommit: (command: Command) => void;
}

function finSets(parts: readonly VehiclePart[]): VehiclePart[] {
  return parts.flatMap((part) => [
    ...(part.kind.type === "fin_set" ? [part] : []),
    ...finSets(part.children),
  ]);
}

export default function PartInspector({
  vehicle,
  preview,
  previewing,
  error,
  onPreview,
  onCommit,
}: Props) {
  const fins = finSets(vehicle.parts).filter(
    (part) => typeof part.kind.span_m === "number",
  );

  return (
    <section className="part-inspector" aria-label="Vehicle parts">
      <h3>Vehicle parts</h3>
      {fins.length === 0 ? (
        <div className="part-preview-muted">No editable fin set in this vehicle.</div>
      ) : (
        fins.map((part) => {
          const span = part.kind.span_m as number;
          return (
            <div className="part-control" key={part.id}>
              <strong>Fin set #{part.id}</strong>
              <DragSlider
                label="Span"
                target={{ id: part.id, param: "span_m", startValue: span }}
                min={Math.max(0.005, span * 0.4)}
                max={Math.max(0.1, span * 2)}
                step={0.0025}
                onPreview={onPreview}
                onCommit={onCommit}
              />
            </div>
          );
        })
      )}
      <div className="part-preview" aria-live="polite">
        {previewing && "Computing read-only preview…"}
        {!previewing && preview && (
          <>
            Preview · apogee {preview.apogee_m.toFixed(1)} m · stability{" "}
            {preview.markers.stability_ignition_cal.toFixed(2)} cal ignition /{" "}
            {preview.markers.stability_burnout_cal.toFixed(2)} cal burnout
          </>
        )}
        {!previewing && error && <span className="part-preview-error">{error}</span>}
      </div>
    </section>
  );
}
