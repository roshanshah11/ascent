import type { Design, MotorInfo } from "../core/types";

interface Props {
  motors: MotorInfo[];
  design: Design;
  onChange: (next: Design) => void;
}

export default function MotorSelector({ motors, design, onChange }: Props) {
  return (
    <section className="property-group motor-properties">
      <div className="property-group-heading"><span>Propulsion</span><span>03</span></div>
      <label className="property-row property-row-stacked">
        <span className="property-label">Motor configuration</span>
        <select
          value={design.motor_designation}
          onChange={(e) => onChange({ ...design, motor_designation: e.target.value })}
        >
          {motors.map((m) => (
            <option key={m.designation} value={m.designation}>
              {m.manufacturer} {m.designation} · {m.total_impulse_ns.toFixed(1)} N·s · {m.burn_time_s.toFixed(2)} s
            </option>
          ))}
        </select>
      </label>
      <div className="property-readonly"><span>Source</span><strong>Bundled motor registry</strong></div>
    </section>
  );
}
