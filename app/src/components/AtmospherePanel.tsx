import type { AtmosphereProfile } from "../core/types";

interface Props {
  profile: AtmosphereProfile | null;
  onImport: (name: string, csv: string) => Promise<void>;
  onClear: () => Promise<void>;
}

export default function AtmospherePanel({ profile, onImport, onClear }: Props) {
  return (
    <section className="property-group atmosphere-properties">
      <div className="property-group-heading"><span>Environment</span><span>04</span></div>
      <div className="atmosphere-state">
        <span className={profile ? "profile-active" : "profile-default"} />
        {profile
          ? `${profile.name} · ${profile.layers.length} levels`
          : "Analytic atmosphere (ISA)"}
      </div>
      <label className="file-import-button">
        <span>Import profile CSV</span>
        <input
          type="file"
          accept=".csv,text/csv"
          onChange={async (event) => {
            const input = event.currentTarget;
            const file = input.files?.[0];
            if (!file) return;
            await onImport(file.name, await file.text());
            input.value = "";
          }}
        />
      </label>
      {profile && <button className="button-secondary property-action" onClick={() => void onClear()}>Use analytic atmosphere</button>}
    </section>
  );
}
