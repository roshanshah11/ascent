import {
  ChartLineUp,
  Cube,
  Gauge,
  RocketLaunch,
  ShieldCheck,
} from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

export type Workspace = "design" | "simulate" | "results" | "review";

export const WORKSPACES: { id: Workspace; label: string; icon: Icon; key: string }[] = [
  { id: "design", label: "Design", icon: Cube, key: "1" },
  { id: "simulate", label: "Simulate", icon: Gauge, key: "2" },
  { id: "results", label: "Results", icon: ChartLineUp, key: "3" },
  { id: "review", label: "Verification", icon: ShieldCheck, key: "4" },
];

interface WorkbenchProps {
  workspace: Workspace;
  onWorkspaceChange: (workspace: Workspace) => void;
  disabledWorkspaces?: Workspace[];
  title: string;
  projectName?: string;
  toolbar?: ReactNode;
  banner?: ReactNode;
  statusbar?: ReactNode;
  children: ReactNode;
}

export default function Workbench({
  workspace,
  onWorkspaceChange,
  disabledWorkspaces = [],
  title,
  projectName,
  toolbar,
  banner,
  statusbar,
  children,
}: WorkbenchProps) {
  const active = WORKSPACES.find((item) => item.id === workspace) ?? WORKSPACES[0];
  return (
    <div className="workbench">
      <header className="workbench-header">
        <div className="workbench-brand">
          <span className="brand-mark"><RocketLaunch size={19} weight="fill" /></span>
          <h1 className="workbench-title">{title}</h1>
          {projectName && (
            <>
              <span className="title-divider" aria-hidden="true" />
              <span className="project-title">{projectName}</span>
              <span className="project-dirty" title="Document has local state" />
            </>
          )}
        </div>
        <div className="workbench-toolbar">{toolbar}</div>
      </header>

      {banner && <div className="workbench-banner">{banner}</div>}

      <div className="workbench-frame">
        <nav className="activity-rail" role="tablist" aria-label="Workspaces">
          {WORKSPACES.map(({ id, label, icon: WorkspaceIcon, key }) => {
            const disabled = disabledWorkspaces.includes(id);
            const selected = workspace === id;
            return (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={selected}
                aria-label={label}
                title={`${label} workspace (${key})`}
                disabled={disabled}
                className={`activity-button${selected ? " active" : ""}`}
                onClick={() => !disabled && onWorkspaceChange(id)}
              >
                <WorkspaceIcon size={20} weight={selected ? "fill" : "regular"} />
                <span className="sr-only">{label}</span>
                <kbd>{key}</kbd>
              </button>
            );
          })}
        </nav>

        <section className="workbench-surface">
          <div className="workspace-contextbar">
            <span className="workspace-name">{active.label}</span>
            <span className="workspace-separator">/</span>
            <span className="workspace-context">{workspace === "design" ? "Vehicle assembly" : workspace === "simulate" ? "Flight playback" : workspace === "results" ? "Post-processing" : "Mission assurance"}</span>
            <span className="workspace-engine">ASCENT SOLVER · LOCAL</span>
          </div>
          <main className="workbench-content">{children}</main>
        </section>
      </div>

      <footer className="statusbar">{statusbar}</footer>
    </div>
  );
}
