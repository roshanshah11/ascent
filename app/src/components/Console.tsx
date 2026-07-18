// Scripting console (v0.3 Step 8): journal-grammar commands in, dispatcher
// results out. Parsing and validation live in Rust — this component only
// sends the line and renders what came back, so the console can never
// mutate anything the GUI couldn't. `journal` dumps the session record.
import { useRef, useState } from "react";
import type { DocumentState } from "../core/types";
import { consoleExec, fetchSessionJournal } from "../ipc";

interface Entry {
  line: string;
  ok: boolean;
  output: string;
}

const HELP = `commands (docs/JOURNAL_FORMAT.md):
  add-part <parent|-> <kind-json>
  remove-part <id>
  set-part-param <id> <param> <value-json>
  set-sim-param <param> <value-json>
  select-motor <designation>
  create-study <"name"> <engine> <seed> <kind-json>
  set-study-param <id> <param> <value-json>
  delete-study <id>
  journal | help`;

export default function Console({
  onDocChange,
}: {
  onDocChange: (doc: DocumentState) => void;
}) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [line, setLine] = useState("");
  // Command history, arrow-key navigable; index counts back from the end.
  const history = useRef<string[]>([]);
  const cursor = useRef(-1);

  const push = (entry: Entry) => setEntries((prev) => [...prev, entry]);

  const run = async () => {
    const cmd = line.trim();
    if (!cmd) return;
    history.current.push(cmd);
    cursor.current = -1;
    setLine("");
    if (cmd === "help") {
      push({ line: cmd, ok: true, output: HELP });
      return;
    }
    if (cmd === "journal") {
      try {
        push({ line: cmd, ok: true, output: (await fetchSessionJournal()).trimEnd() });
      } catch (e) {
        push({ line: cmd, ok: false, output: String(e) });
      }
      return;
    }
    try {
      const doc = await consoleExec(cmd);
      onDocChange(doc);
      push({ line: cmd, ok: true, output: "ok" });
    } catch (e) {
      push({ line: cmd, ok: false, output: String(e) });
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      void run();
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      const h = history.current;
      if (h.length === 0) return;
      e.preventDefault();
      if (e.key === "ArrowUp") {
        cursor.current = cursor.current < 0 ? h.length - 1 : Math.max(0, cursor.current - 1);
        setLine(h[cursor.current]);
      } else if (cursor.current >= 0) {
        cursor.current += 1;
        if (cursor.current >= h.length) {
          cursor.current = -1;
          setLine("");
        } else {
          setLine(h[cursor.current]);
        }
      }
    }
  };

  return (
    <section
      style={{
        marginTop: 16,
        fontFamily: "ui-monospace, monospace",
        fontSize: 12,
        maxWidth: 720,
      }}
    >
      <div
        style={{
          border: "1px solid #30363d",
          borderRadius: 6,
          padding: 8,
          maxHeight: 180,
          overflowY: "auto",
        }}
      >
        {entries.length === 0 && (
          <div style={{ color: "#9aa1ab" }}>Ascent console — type `help` for the grammar.</div>
        )}
        {entries.map((e, i) => (
          <div key={i}>
            <div style={{ color: "#58a6ff" }}>&gt; {e.line}</div>
            <pre
              style={{
                margin: "0 0 4px 12px",
                whiteSpace: "pre-wrap",
                color: e.ok ? "#9aa1ab" : "#c74b3c",
              }}
            >
              {e.output}
            </pre>
          </div>
        ))}
      </div>
      <input
        value={line}
        onChange={(e) => setLine(e.target.value)}
        onKeyDown={onKeyDown}
        placeholder="set-sim-param cd 0.7"
        aria-label="console input"
        style={{ width: "100%", marginTop: 4, fontFamily: "inherit", fontSize: 12 }}
      />
    </section>
  );
}
