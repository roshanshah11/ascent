// Thin 3D view over vehicleToMesh (v0.3 Step 3): renders the same tree
// that drives Barrowman and the mass rollup. Hand-rolled canvas
// projector — deliberately renderer-agnostic and disposable: the mesh
// module owns the geometry, and the redesign pass can swap this file for
// WebGPU/wgpu without touching mesh.ts. No external 3D dependency (zero
// new deps rule). Orbit by dragging.
import { useEffect, useRef, useState } from "react";
import { stackHeightM, vehicleToMesh } from "../core/mesh";
import type { Vehicle } from "../core/types";

const W = 300;
const H = 380;
const PART_COLOR: Record<string, string> = {
  nose_cone: "#c74b3c",
  body_tube: "#d8dbe0",
  transition: "#d8dbe0",
};

export default function Viewport3D({ vehicle }: { vehicle: Vehicle }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [orbit, setOrbit] = useState({ yaw: 0.6, pitch: 0.25 });
  const dragRef = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const mesh = vehicleToMesh(vehicle);
    const height = stackHeightM(vehicle);
    if (height <= 0) return;
    const scale = (H * 0.7) / height; // fit the stack in the frame
    const cy = height / 2;

    const cosY = Math.cos(orbit.yaw);
    const sinY = Math.sin(orbit.yaw);
    const cosP = Math.cos(orbit.pitch);
    const sinP = Math.sin(orbit.pitch);

    // Model → camera: yaw about Y, then pitch about X. Orthographic —
    // rockets are long and thin; perspective adds nothing at this size.
    const project = (i: number) => {
      const x = mesh.positions[3 * i];
      const y = mesh.positions[3 * i + 1] - cy;
      const z = mesh.positions[3 * i + 2];
      const rx = x * cosY + z * sinY;
      const rz = -x * sinY + z * cosY;
      const ry = y * cosP - rz * sinP;
      const depth = y * sinP + rz * cosP;
      return { sx: W / 2 + rx * scale, sy: H / 2 - ry * scale, depth };
    };

    const projected = Array.from({ length: mesh.positions.length / 3 }, (_, i) => project(i));

    // Painter's algorithm over all triangles, flat-shaded by face normal.
    type Face = { a: number; b: number; c: number; depth: number; part: string };
    const faces: Face[] = [];
    for (const part of mesh.parts) {
      for (let i = part.start; i < part.start + part.count; i += 3) {
        const [a, b, c] = [mesh.indices[i], mesh.indices[i + 1], mesh.indices[i + 2]];
        faces.push({
          a,
          b,
          c,
          depth: (projected[a].depth + projected[b].depth + projected[c].depth) / 3,
          part: part.kind,
        });
      }
    }
    faces.sort((f, g) => f.depth - g.depth);

    ctx.clearRect(0, 0, W, H);
    for (const f of faces) {
      const pa = projected[f.a];
      const pb = projected[f.b];
      const pc = projected[f.c];
      // Screen-space normal Z for a cheap headlight shade.
      const cross =
        (pb.sx - pa.sx) * (pc.sy - pa.sy) - (pb.sy - pa.sy) * (pc.sx - pa.sx);
      const facing = cross < 0;
      if (!facing && f.part === "fin_set") {
        // Fins are plates: render both sides. Revolved parts cull backfaces.
      } else if (!facing) {
        continue;
      }
      const base = PART_COLOR[f.part] ?? "#c74b3c";
      const area = Math.abs(cross);
      const shade = Math.min(1, 0.45 + area / 2400);
      ctx.fillStyle = base;
      ctx.globalAlpha = shade;
      ctx.beginPath();
      ctx.moveTo(pa.sx, pa.sy);
      ctx.lineTo(pb.sx, pb.sy);
      ctx.lineTo(pc.sx, pc.sy);
      ctx.closePath();
      ctx.fill();
    }
    ctx.globalAlpha = 1;
    ctx.fillStyle = "#9aa1ab";
    ctx.font = "12px sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(`${vehicle.name} · drag to orbit`, W / 2, H - 8);
  }, [vehicle, orbit]);

  return (
    <canvas
      ref={canvasRef}
      width={W}
      height={H}
      role="img"
      aria-label={`3D view of ${vehicle.name}`}
      style={{ cursor: "grab", touchAction: "none" }}
      onPointerDown={(e) => {
        dragRef.current = { x: e.clientX, y: e.clientY };
        (e.target as HTMLCanvasElement).setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        if (!dragRef.current) return;
        const dx = e.clientX - dragRef.current.x;
        const dy = e.clientY - dragRef.current.y;
        dragRef.current = { x: e.clientX, y: e.clientY };
        setOrbit((o) => ({
          yaw: o.yaw + dx * 0.01,
          pitch: Math.max(-1.4, Math.min(1.4, o.pitch + dy * 0.01)),
        }));
      }}
      onPointerUp={() => {
        dragRef.current = null;
      }}
    />
  );
}
