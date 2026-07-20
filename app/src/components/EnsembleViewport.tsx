// Dispersion-ensemble viewport (v0.5 Step 7). A thin R3F consumer of the
// pure geometry in core/ensemble.ts: one LineSegments buffer holds every
// member's schematic arc (a single draw call, the instancing intent), a
// points cloud marks landings, a line loop traces the landing ellipse, and
// translucent rings mark the p5/p50/p95 apogee shells. frameloop="demand"
// keeps it determinism-friendly. Render-layer invariant: this file
// dispatches zero commands; it consumes a DispersionSummary snapshot only.
import { CameraControls, Html } from "@react-three/drei";
import { Canvas } from "@react-three/fiber";
import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import {
  apogeeShells,
  ensembleArcs,
  ensembleBounds,
  landingEllipseRing,
  landingPoints,
} from "../core/ensemble";
import type { DispersionSummary } from "../core/types";

const ARC_SEGMENTS = 24;
const ELLIPSE_SEGMENTS = 64;

/** Build a LineSegments geometry from concatenated per-member arc points:
 *  consecutive points within a member become a segment; members don't join. */
function arcsGeometry(summary: DispersionSummary): THREE.BufferGeometry {
  const { points, perMember } = ensembleArcs(summary, ARC_SEGMENTS);
  const members = summary.runs.length;
  // (perMember - 1) segments per member, 2 vertices each, 3 floats each.
  const segsPerMember = perMember - 1;
  const positions = new Float32Array(members * segsPerMember * 2 * 3);
  let w = 0;
  for (let m = 0; m < members; m++) {
    const base = m * perMember;
    for (let s = 0; s < segsPerMember; s++) {
      const a = points[base + s];
      const b = points[base + s + 1];
      positions[w++] = a.x;
      positions[w++] = a.y;
      positions[w++] = a.z;
      positions[w++] = b.x;
      positions[w++] = b.y;
      positions[w++] = b.z;
    }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  return geo;
}

function pointsGeometry(summary: DispersionSummary): THREE.BufferGeometry {
  const pts = landingPoints(summary);
  const positions = new Float32Array(pts.length * 3);
  pts.forEach((p, i) => {
    positions[i * 3] = p.x;
    positions[i * 3 + 1] = p.y;
    positions[i * 3 + 2] = p.z;
  });
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  return geo;
}

function ellipseGeometry(summary: DispersionSummary): THREE.BufferGeometry {
  const ring = landingEllipseRing(summary, ELLIPSE_SEGMENTS);
  const positions = new Float32Array(ring.length * 3);
  ring.forEach((p, i) => {
    positions[i * 3] = p.x;
    positions[i * 3 + 1] = p.y;
    positions[i * 3 + 2] = p.z;
  });
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  return geo;
}

function EnsembleScene({ summary }: { summary: DispersionSummary }) {
  const arcs = useMemo(() => arcsGeometry(summary), [summary]);
  const landings = useMemo(() => pointsGeometry(summary), [summary]);
  const ellipse = useMemo(() => ellipseGeometry(summary), [summary]);
  const shells = useMemo(() => apogeeShells(summary), [summary]);
  const bounds = useMemo(() => ensembleBounds(summary), [summary]);

  useEffect(
    () => () => {
      arcs.dispose();
      landings.dispose();
      ellipse.dispose();
    },
    [arcs, landings, ellipse],
  );

  const shellRadius = bounds ? Math.max(bounds.max_range_m, 1) * 0.6 : 1;

  return (
    <>
      <lineSegments geometry={arcs}>
        <lineBasicMaterial color="#4da3e8" transparent opacity={0.35} />
      </lineSegments>

      <points geometry={landings}>
        <pointsMaterial color="#e8a33d" size={0.04} sizeAttenuation={false} />
      </points>

      <lineLoop geometry={ellipse}>
        <lineBasicMaterial color="#e85d5d" />
      </lineLoop>

      {shells.map((shell) => (
        <group key={shell.label} position={[0, shell.apogee_m, 0]}>
          <mesh rotation={[Math.PI / 2, 0, 0]}>
            <ringGeometry args={[shellRadius * 0.98, shellRadius, 48]} />
            <meshBasicMaterial
              color="#7bd88f"
              transparent
              opacity={0.5}
              side={THREE.DoubleSide}
            />
          </mesh>
          <Html position={[shellRadius, 0, 0]} style={{ pointerEvents: "none" }}>
            <span
              style={{
                color: "#7bd88f",
                fontSize: 11,
                fontFamily: "var(--ascent-font-mono, monospace)",
              }}
            >
              {shell.label} · {shell.apogee_m.toFixed(0)} m
            </span>
          </Html>
        </group>
      ))}
    </>
  );
}

export default function EnsembleViewport({
  summary,
}: {
  summary: DispersionSummary | null;
}) {
  const [controls, setControls] = useState<CameraControls | null>(null);
  const bounds = useMemo(
    () => (summary ? ensembleBounds(summary) : null),
    [summary],
  );

  useEffect(() => {
    if (!controls || !bounds) return;
    const span = Math.max(bounds.max_range_m, bounds.max_apogee_m, 1);
    void controls.setLookAt(
      span * 1.4,
      bounds.max_apogee_m * 0.9,
      span * 1.4,
      bounds.max_range_m / 2,
      bounds.max_apogee_m / 2,
      0,
      false,
    );
  }, [controls, bounds]);

  if (!summary || summary.runs.length === 0) {
    return (
      <div className="ensemble-viewport ensemble-viewport--empty">
        <p>Run a dispersion study to see the ensemble.</p>
      </div>
    );
  }

  const span = bounds ? Math.max(bounds.max_range_m, bounds.max_apogee_m, 1) : 1;

  return (
    <div
      className="ensemble-viewport"
      aria-label={`Dispersion ensemble of ${summary.runs.length} flights`}
      role="img"
    >
      <Canvas
        frameloop="demand"
        camera={{
          fov: 45,
          near: 0.1,
          far: span * 20,
          position: [span * 1.4, span * 0.7, span * 1.4],
        }}
      >
        <ambientLight intensity={0.8} />
        <directionalLight position={[3, 5, 4]} intensity={0.8} />
        <gridHelper args={[span * 2, 20, "#334", "#223"]} />
        <EnsembleScene summary={summary} />
        <CameraControls ref={setControls} makeDefault />
      </Canvas>
    </div>
  );
}
