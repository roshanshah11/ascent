// Part inspector: every editable Design field. Emits whole-Design updates.
import type { Design } from "../core/types";

interface Props {
  design: Design;
  onChange: (next: Design) => void;
}

function Num({
  label,
  value,
  step,
  onChange,
}: {
  label: string;
  value: number;
  step?: number;
  onChange: (v: number) => void;
}) {
  return (
    <label style={{ display: "block", margin: "6px 0" }}>
      <span style={{ display: "inline-block", width: 150 }}>{label}</span>
      <input
        type="number"
        value={value}
        step={step ?? 1}
        style={{ width: 90 }}
        onChange={(e) => {
          const v = Number(e.target.value);
          if (Number.isFinite(v)) onChange(v);
        }}
      />
    </label>
  );
}

export default function Inspector({ design, onChange }: Props) {
  return (
    <div>
      <h3>Rocket</h3>
      <label style={{ display: "block", margin: "6px 0" }}>
        <span style={{ display: "inline-block", width: 150 }}>Name</span>
        <input
          value={design.name}
          onChange={(e) => onChange({ ...design, name: e.target.value })}
        />
      </label>
      <Num
        label="Dry mass (g)"
        value={design.dry_mass_g}
        onChange={(v) => onChange({ ...design, dry_mass_g: v })}
      />
      <Num
        label="Diameter (mm)"
        value={design.diameter_mm}
        onChange={(v) => onChange({ ...design, diameter_mm: v })}
      />
      <Num
        label="Drag Cd"
        value={design.cd}
        step={0.01}
        onChange={(v) => onChange({ ...design, cd: v })}
      />
      <Num
        label="Rail length (m)"
        value={design.rail_length_m}
        step={0.1}
        onChange={(v) => onChange({ ...design, rail_length_m: v })}
      />
      <h3>Recovery</h3>
      <label style={{ display: "block", margin: "6px 0" }}>
        <span style={{ display: "inline-block", width: 150 }}>Parachute</span>
        <input
          type="checkbox"
          checked={design.chute.enabled}
          onChange={(e) =>
            onChange({ ...design, chute: { ...design.chute, enabled: e.target.checked } })
          }
        />
      </label>
      {design.chute.enabled && (
        <>
          <Num
            label="Chute diameter (cm)"
            value={design.chute.diameter_cm}
            onChange={(v) =>
              onChange({ ...design, chute: { ...design.chute, diameter_cm: v } })
            }
          />
          <Num
            label="Chute Cd"
            value={design.chute.cd}
            step={0.05}
            onChange={(v) => onChange({ ...design, chute: { ...design.chute, cd: v } })}
          />
        </>
      )}
    </div>
  );
}
