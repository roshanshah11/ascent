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
    <label className="property-row">
      <span className="property-label">{label}</span>
      <span className="property-input-wrap">
        <input
          type="number"
          value={value}
          step={step ?? 1}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (Number.isFinite(v)) onChange(v);
          }}
        />
      </span>
    </label>
  );
}

export default function Inspector({ design, onChange }: Props) {
  const dualDeploy = design.chute.main_deploy_altitude_m != null;

  return (
    <div className="inspector-form">
      <section className="property-group">
      <div className="property-group-heading"><span>Vehicle</span><span>01</span></div>
      <label className="property-row">
        <span className="property-label">Name</span>
        <input
          className="property-text"
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
      </section>
      <section className="property-group">
      <div className="property-group-heading"><span>Recovery system</span><span>02</span></div>
      <label className="property-row property-toggle-row">
        <span className="property-label">Parachute</span>
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
          <label className="property-row property-toggle-row">
            <span className="property-label">Dual deploy</span>
            <input
              type="checkbox"
              checked={dualDeploy}
              onChange={(e) =>
                onChange({
                  ...design,
                  chute: e.target.checked
                    ? {
                        ...design.chute,
                        main_deploy_altitude_m: 60,
                        drogue_diameter_cm: 8,
                        drogue_cd: 0.8,
                      }
                    : {
                        ...design.chute,
                        main_deploy_altitude_m: null,
                        drogue_diameter_cm: null,
                        drogue_cd: null,
                      },
                })
              }
            />
          </label>
          {dualDeploy && (
            <>
              <Num
                label="Main deploy AGL (m)"
                value={design.chute.main_deploy_altitude_m ?? 60}
                onChange={(v) =>
                  onChange({
                    ...design,
                    chute: { ...design.chute, main_deploy_altitude_m: v },
                  })
                }
              />
              <Num
                label="Drogue diameter (cm)"
                value={design.chute.drogue_diameter_cm ?? 8}
                onChange={(v) =>
                  onChange({
                    ...design,
                    chute: { ...design.chute, drogue_diameter_cm: v },
                  })
                }
              />
              <Num
                label="Drogue Cd"
                value={design.chute.drogue_cd ?? 0.8}
                step={0.05}
                onChange={(v) =>
                  onChange({
                    ...design,
                    chute: { ...design.chute, drogue_cd: v },
                  })
                }
              />
            </>
          )}
        </>
      )}
      </section>
    </div>
  );
}
