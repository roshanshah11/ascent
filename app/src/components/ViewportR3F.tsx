// React Three Fiber vehicle viewport (v0.4). Replaces the hand-rolled
// canvas projector: same pure mesh module (core/mesh.ts, zero renderer
// imports) feeding real BufferGeometry, drei CameraControls for orbit,
// CP/CG stations from the read-only `get_vehicle_markers` query drawn as
// overlay rings. frameloop="demand": frames render only when the tree or
// the camera changes — determinism-friendly, no free-running loop.
// Render-layer invariant: this file dispatches zero commands; it consumes
// snapshots (vehicle prop) and the marker query result only.
import { CameraControls, Html } from "@react-three/drei";
import { Canvas } from "@react-three/fiber";
import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { stackHeightM, vehicleToMesh } from "../core/mesh";
import { markerY, meshGroups, PART_MATERIALS } from "../core/meshGroups";
import type { Vehicle, VehicleMarkers } from "../core/types";

const MATERIALS = PART_MATERIALS.map(
  (m) =>
    new THREE.MeshStandardMaterial({
      color: m.color,
      metalness: 0.15,
      roughness: 0.6,
      side: m.kind === "fin_set" ? THREE.DoubleSide : THREE.FrontSide,
    }),
);

function RocketBody({ vehicle }: { vehicle: Vehicle }) {
  const geometry = useMemo(() => {
    const mesh = vehicleToMesh(vehicle);
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(mesh.positions, 3));
    geo.setAttribute("normal", new THREE.BufferAttribute(mesh.normals, 3));
    geo.setIndex(new THREE.BufferAttribute(mesh.indices, 1));
    for (const g of meshGroups(mesh)) {
      geo.addGroup(g.start, g.count, g.materialIndex);
    }
    return geo;
  }, [vehicle]);

  useEffect(() => () => geometry.dispose(), [geometry]);

  return <mesh geometry={geometry} material={MATERIALS} />;
}

/** Horizontal ring around the airframe at a station, with a label. */
function StationMarker({
  y,
  radius,
  color,
  label,
}: {
  y: number;
  radius: number;
  color: string;
  label: string;
}) {
  return (
    <group position={[0, y, 0]}>
      <mesh rotation={[Math.PI / 2, 0, 0]}>
        <torusGeometry args={[radius * 1.35, radius * 0.06, 8, 48]} />
        <meshBasicMaterial color={color} />
      </mesh>
      <Html
        position={[radius * 2.2, 0, 0]}
        style={{ pointerEvents: "none", whiteSpace: "nowrap" }}
      >
        <span style={{ color, fontSize: 11, fontFamily: "var(--ascent-font-mono, monospace)" }}>
          {label}
        </span>
      </Html>
    </group>
  );
}

export default function ViewportR3F({
  vehicle,
  markers,
}: {
  vehicle: Vehicle;
  markers: VehicleMarkers | null;
}) {
  const height = stackHeightM(vehicle);
  const [controls, setControls] = useState<CameraControls | null>(null);

  // Re-frame when the stack height changes (part added/removed/resized).
  useEffect(() => {
    if (!controls || height <= 0) return;
    void controls.setLookAt(height * 1.6, height * 0.7, height * 1.6, 0, height / 2, 0, false);
  }, [controls, height]);

  if (height <= 0) return null;
  const radius = markers ? markers.diameter_m / 2 : height * 0.03;

  return (
    <div className="viewport-r3f" aria-label={`3D view of ${vehicle.name}`} role="img">
      <Canvas
        frameloop="demand"
        camera={{ fov: 40, near: 0.001, far: 100, position: [height * 1.6, height * 0.7, height * 1.6] }}
      >
        <ambientLight intensity={0.7} />
        <directionalLight position={[3, 5, 4]} intensity={1.1} />
        <directionalLight position={[-4, 2, -3]} intensity={0.3} />
        <RocketBody vehicle={vehicle} />
        {markers && (
          <>
            <StationMarker
              y={markerY(markers.length_m, markers.cp_from_nose_m)}
              radius={radius}
              color="#e8a33d"
              label={`CP · ${markers.stability_ignition_cal.toFixed(2)} cal`}
            />
            <StationMarker
              y={markerY(markers.length_m, markers.cg_ignition_from_nose_m)}
              radius={radius}
              color="#4da3e8"
              label="CG ignition"
            />
            <StationMarker
              y={markerY(markers.length_m, markers.cg_burnout_from_nose_m)}
              radius={radius}
              color="#7bd88f"
              label="CG burnout"
            />
          </>
        )}
        <CameraControls ref={setControls} makeDefault />
      </Canvas>
    </div>
  );
}
