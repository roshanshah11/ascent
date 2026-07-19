// CSS-grid workbench shell (v0.4 Step 10). Pure layout + workspace tabs —
// it renders whatever content the caller hands it per workspace and never
// touches the document store itself, so it dispatches zero commands. The
// mission-control visual language lives in styles/theme.css as CSS custom
// properties; this component only wires class names.
import type { ReactNode } from "react";

export type Workspace = "design" | "simulate" | "results" | "review";

export const WORKSPACES: { id: Workspace; label: string }[] = [
  { id: "design", label: "Design" },
  { id: "simulate", label: "Simulate" },
  { id: "results", label: "Results" },
  { id: "review", label: "Review" },
];

interface WorkbenchProps {
  workspace: Workspace;
  onWorkspaceChange: (workspace: Workspace) => void;
  disabledWorkspaces?: Workspace[];
  title: string;
  toolbar?: ReactNode;
  banner?: ReactNode;
  children: ReactNode;
}

export default function Workbench({
  workspace,
  onWorkspaceChange,
  disabledWorkspaces = [],
  title,
  toolbar,
  banner,
  children,
}: WorkbenchProps) {
  return (
    <div className="workbench">
      <header className="workbench-header">
        <h1 className="workbench-title">{title}</h1>
        <div className="workbench-toolbar">{toolbar}</div>
      </header>
      <nav className="workbench-tabs" role="tablist" aria-label="Workspaces">
        {WORKSPACES.map(({ id, label }) => {
          const disabled = disabledWorkspaces.includes(id);
          const active = workspace === id;
          return (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={active}
              disabled={disabled}
              className={`workbench-tab${active ? " active" : ""}`}
              onClick={() => !disabled && onWorkspaceChange(id)}
            >
              {label}
            </button>
          );
        })}
      </nav>
      {banner && <div className="workbench-banner">{banner}</div>}
      <main className="workbench-content">{children}</main>
    </div>
  );
}
