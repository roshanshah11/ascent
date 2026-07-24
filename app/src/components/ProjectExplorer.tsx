import {
  CaretDown,
  ChartLine,
  CloudSun,
  Cube,
  Database,
  Flask,
  Rocket,
} from "@phosphor-icons/react";
import type { AtmosphereProfile, Study, Vehicle, VehiclePart } from "../core/types";

interface Props {
  vehicle: Vehicle;
  studies: Study[];
  atmosphere: AtmosphereProfile | null;
  selected: string;
  onSelect: (id: string) => void;
}

const titleCase = (value: string) =>
  value
    .replaceAll("_", " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());

function PartNode({
  part,
  depth,
  selected,
  onSelect,
}: {
  part: VehiclePart;
  depth: number;
  selected: string;
  onSelect: (id: string) => void;
}) {
  const id = `part-${part.id}`;
  const kind = typeof part.kind.type === "string" ? part.kind.type : "component";
  return (
    <li>
      <button
        type="button"
        className={`tree-row${selected === id ? " selected" : ""}`}
        style={{ "--tree-depth": depth } as React.CSSProperties}
        onClick={() => onSelect(id)}
      >
        <span className="tree-disclosure" aria-hidden="true">
          {part.children.length > 0 ? <CaretDown size={10} weight="bold" /> : null}
        </span>
        <Cube size={14} weight="duotone" />
        <span className="tree-label">{titleCase(kind)}</span>
        <span className="tree-meta">{part.id}</span>
      </button>
      {part.children.length > 0 && (
        <ul className="tree-list">
          {part.children.map((child) => (
            <PartNode
              key={child.id}
              part={child}
              depth={depth + 1}
              selected={selected}
              onSelect={onSelect}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

export default function ProjectExplorer({
  vehicle,
  studies,
  atmosphere,
  selected,
  onSelect,
}: Props) {
  return (
    <div className="project-explorer">
      <div className="panel-heading">
        <div>
          <span className="panel-eyebrow">Model</span>
          <h2>Project Explorer</h2>
        </div>
        <span className="panel-count">{vehicle.parts.length}</span>
      </div>

      <div className="tree-section">
        <button
          type="button"
          className={`tree-row tree-root${selected === "vehicle" ? " selected" : ""}`}
          onClick={() => onSelect("vehicle")}
        >
          <CaretDown size={10} weight="bold" />
          <Rocket size={15} weight="duotone" />
          <span className="tree-label">{vehicle.name}</span>
        </button>
        <ul className="tree-list">
          {vehicle.parts.map((part) => (
            <PartNode
              key={part.id}
              part={part}
              depth={1}
              selected={selected}
              onSelect={onSelect}
            />
          ))}
        </ul>
      </div>

      <div className="tree-section">
        <div className="tree-section-label">Analysis pipeline</div>
        <button
          type="button"
          className={`tree-row${selected === "flight-model" ? " selected" : ""}`}
          onClick={() => onSelect("flight-model")}
        >
          <span className="tree-disclosure" aria-hidden="true" />
          <Flask size={14} weight="duotone" />
          <span className="tree-label">Flight model</span>
          <span
            className="tree-meta"
            title="Reduced rotational model (13-state): pitch/yaw restoring only — no roll, no rotational damping; attitude frozen during descent"
          >
            reduced rot.
          </span>
        </button>
        <button
          type="button"
          className={`tree-row${selected === "atmosphere" ? " selected" : ""}`}
          onClick={() => onSelect("atmosphere")}
        >
          <span className="tree-disclosure" aria-hidden="true" />
          <CloudSun size={14} weight="duotone" />
          <span className="tree-label">Atmosphere</span>
          <span className="tree-meta">{atmosphere ? atmosphere.layers.length : "ISA"}</span>
        </button>
        <button
          type="button"
          className={`tree-row${selected === "motor-data" ? " selected" : ""}`}
          onClick={() => onSelect("motor-data")}
        >
          <span className="tree-disclosure" aria-hidden="true" />
          <Database size={14} weight="duotone" />
          <span className="tree-label">Motor data</span>
          <span className="tree-meta">local</span>
        </button>
      </div>

      <div className="tree-section">
        <div className="tree-section-label">Studies</div>
        {studies.length === 0 ? (
          <div className="tree-empty">No saved studies</div>
        ) : (
          studies.map((study) => (
            <button
              type="button"
              key={study.id}
              className={`tree-row${selected === `study-${study.id}` ? " selected" : ""}`}
              onClick={() => onSelect(`study-${study.id}`)}
            >
              <span className="tree-disclosure" aria-hidden="true" />
              <ChartLine size={14} weight="duotone" />
              <span className="tree-label">{study.name}</span>
              <span className={`tree-status ${study.results ? "complete" : "pending"}`} />
            </button>
          ))
        )}
      </div>
    </div>
  );
}
