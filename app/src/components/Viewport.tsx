// Side-view rocket rendering from the parametric design. Placeholder
// visuals; geometry-driven so it stays correct under any redesign.
import type { Design } from "../core/types";

export default function Viewport({ design }: { design: Design }) {
  // Simple proportional side view: nose + body + fins + chute marker.
  const bodyLen = 220;
  const noseLen = 70;
  const d = Math.max(10, Math.min(60, design.diameter_mm * 1.6));
  const cx = 150;
  const top = 30;

  return (
    <svg width="300" height="380" role="img" aria-label={`Side view of ${design.name}`}>
      <title>{design.name}</title>
      {/* nose cone */}
      <polygon
        points={`${cx},${top} ${cx - d / 2},${top + noseLen} ${cx + d / 2},${top + noseLen}`}
        fill="#c74b3c"
      />
      {/* body tube */}
      <rect x={cx - d / 2} y={top + noseLen} width={d} height={bodyLen} fill="#d8dbe0" />
      {/* fins */}
      <polygon
        points={`${cx - d / 2},${top + noseLen + bodyLen - 55} ${cx - d / 2 - 34},${top + noseLen + bodyLen + 6} ${cx - d / 2},${top + noseLen + bodyLen}`}
        fill="#c74b3c"
      />
      <polygon
        points={`${cx + d / 2},${top + noseLen + bodyLen - 55} ${cx + d / 2 + 34},${top + noseLen + bodyLen + 6} ${cx + d / 2},${top + noseLen + bodyLen}`}
        fill="#c74b3c"
      />
      {/* chute marker */}
      {design.chute.enabled && (
        <circle cx={cx} cy={top + noseLen + 30} r={8} fill="#e8a33d">
          <title>parachute: {design.chute.diameter_cm} cm</title>
        </circle>
      )}
      <text x={cx} y={top + noseLen + bodyLen + 40} textAnchor="middle" fill="#9aa1ab" fontSize="12">
        {design.name} · ⌀{design.diameter_mm} mm · {design.dry_mass_g} g dry
      </text>
    </svg>
  );
}
