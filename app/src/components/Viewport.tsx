import type { Design, Vehicle, VehicleMarkers, VehiclePart } from "../core/types";

export default function Viewport({
  design,
  markers,
  vehicle,
}: {
  design: Design;
  markers?: VehicleMarkers | null;
  vehicle?: Vehicle | null;
}) {
  const noseX = 150;
  const shoulderX = 270;
  const tailX = 790;
  const centerY = 250;
  const radius = Math.max(22, Math.min(54, design.diameter_mm * 1.35));
  const top = centerY - radius;
  const bottom = centerY + radius;
  const markerX = (station: number) =>
    noseX + Math.max(0, Math.min(1, station / Math.max(markers?.length_m ?? 1, 0.001))) * (tailX - noseX);
  const fin = findFin(vehicle?.parts ?? []);
  const finRootPx = finiteGeometry(fin?.root_chord_m, 0.04) * 2500;
  const finTipPx = finiteGeometry(fin?.tip_chord_m, 0.02) * 2500;
  const finSpanPx = finiteGeometry(fin?.span_m, 0.03) * 2500;
  const finSweepPx = finiteGeometry(fin?.sweep_m, 0) * 2500;
  const finLeadingX = tailX - finRootPx;

  return (
    <svg
      className="vehicle-drawing"
      viewBox="0 0 1000 520"
      role="img"
      aria-label={`Engineering side elevation of ${design.name}`}
      preserveAspectRatio="xMidYMid meet"
    >
      <title>{design.name} side elevation</title>
      <defs>
        <pattern id="minor-grid" width="20" height="20" patternUnits="userSpaceOnUse">
          <path d="M 20 0 L 0 0 0 20" fill="none" stroke="var(--grid-minor)" strokeWidth="1" />
        </pattern>
        <pattern id="major-grid" width="100" height="100" patternUnits="userSpaceOnUse">
          <rect width="100" height="100" fill="url(#minor-grid)" />
          <path d="M 100 0 L 0 0 0 100" fill="none" stroke="var(--grid-major)" strokeWidth="1" />
        </pattern>
        <linearGradient id="airframe-fill" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0" stopColor="#d8d9dc" />
          <stop offset="0.48" stopColor="#8e9299" />
          <stop offset="0.52" stopColor="#747981" />
          <stop offset="1" stopColor="#c8cacf" />
        </linearGradient>
        <linearGradient id="nose-fill" x1="0" x2="1">
          <stop offset="0" stopColor="#813f38" />
          <stop offset="0.65" stopColor="#b2574d" />
          <stop offset="1" stopColor="#783a35" />
        </linearGradient>
      </defs>

      <rect width="1000" height="520" fill="url(#major-grid)" />
      <line x1="72" y1={centerY} x2="928" y2={centerY} className="drawing-centerline" />
      <text x="78" y={centerY - 8} className="drawing-axis-label">CL</text>

      <g className="rocket-geometry">
        <path
          d={`M ${noseX} ${centerY} C ${noseX + 34} ${top + 7}, ${shoulderX - 28} ${top}, ${shoulderX} ${top} L ${shoulderX} ${bottom} C ${shoulderX - 28} ${bottom}, ${noseX + 34} ${bottom - 7}, ${noseX} ${centerY} Z`}
          fill="url(#nose-fill)"
          stroke="#d4776c"
          strokeWidth="1.25"
        />
        <rect
          x={shoulderX}
          y={top}
          width={tailX - shoulderX}
          height={radius * 2}
          fill="url(#airframe-fill)"
          stroke="#e2e4e8"
          strokeWidth="1.25"
        />
        <line x1={shoulderX + 18} y1={top} x2={shoulderX + 18} y2={bottom} className="drawing-seam" />
        <line x1={tailX - 52} y1={top} x2={tailX - 52} y2={bottom} className="drawing-seam" />
        <path
          d={`M ${finLeadingX} ${bottom - 2} L ${finLeadingX + finSweepPx} ${bottom + finSpanPx} L ${finLeadingX + finSweepPx + finTipPx} ${bottom + finSpanPx} L ${tailX} ${bottom - 2} Z`}
          fill="#9b4942"
          stroke="#d4776c"
          strokeWidth="1.25"
        />
        <path
          d={`M ${finLeadingX} ${top + 2} L ${finLeadingX + finSweepPx} ${top - finSpanPx} L ${finLeadingX + finSweepPx + finTipPx} ${top - finSpanPx} L ${tailX} ${top + 2} Z`}
          fill="#6f3531"
          stroke="#b65b53"
          strokeWidth="1.25"
          opacity="0.7"
        />
        <rect x={tailX} y={top + 8} width="18" height={radius * 2 - 16} fill="#3b3f45" stroke="#777d86" />
      </g>

      {design.chute.enabled && (
        <g transform={`translate(${shoulderX + 78}, ${centerY})`}>
          <circle r="11" className="drawing-recovery-marker" />
          <path d="M -6 2 Q 0 -7 6 2 M 0 -7 V 6" className="drawing-recovery-icon" />
          <text x="0" y="-19" textAnchor="middle" className="drawing-marker-label">RECOVERY</text>
        </g>
      )}

      {markers && (
        <g className="drawing-markers">
          <g transform={`translate(${markerX(markers.cg_ignition_from_nose_m)}, ${centerY})`}>
            <line y1={-radius - 46} y2={radius + 46} className="marker-line cg" />
            <circle r="7" className="marker-point cg" />
            <text y={-radius - 56} textAnchor="middle" className="marker-label cg">CG · IGN</text>
          </g>
          <g transform={`translate(${markerX(markers.cp_from_nose_m)}, ${centerY})`}>
            <line y1={-radius - 46} y2={radius + 46} className="marker-line cp" />
            <circle r="7" className="marker-point cp" />
            <text y={radius + 64} textAnchor="middle" className="marker-label cp">CP</text>
          </g>
        </g>
      )}

      <g className="drawing-dimensions">
        <line x1={noseX} y1="438" x2={tailX + 18} y2="438" />
        <line x1={noseX} y1="425" x2={noseX} y2="451" />
        <line x1={tailX + 18} y1="425" x2={tailX + 18} y2="451" />
        <path d={`M ${noseX + 2} 438 l 12 -5 v 10 z M ${tailX + 16} 438 l -12 -5 v 10 z`} />
        <text x={(noseX + tailX + 18) / 2} y="430" textAnchor="middle">REFERENCE LENGTH · FIT TO VIEW</text>

        <line x1="872" y1={top} x2="872" y2={bottom} />
        <line x1="860" y1={top} x2="884" y2={top} />
        <line x1="860" y1={bottom} x2="884" y2={bottom} />
        <text x="890" y={centerY + 4}>⌀ {design.diameter_mm.toFixed(1)} mm</text>
      </g>

      <g className="drawing-titleblock" transform="translate(660, 464)">
        <rect width="272" height="38" />
        <text x="10" y="15" className="titleblock-name">{design.name.toUpperCase()}</text>
        <text x="10" y="29">SIDE ELEVATION · CONTROLLED</text>
        <text x="255" y="29" textAnchor="end">A-01</text>
      </g>
    </svg>
  );
}

function findFin(parts: VehiclePart[]): Record<string, unknown> | null {
  for (const part of parts) {
    if (part.kind.type === "fin_set") return part.kind;
    const nested = findFin(part.children);
    if (nested) return nested;
  }
  return null;
}

function finiteGeometry(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : fallback;
}
