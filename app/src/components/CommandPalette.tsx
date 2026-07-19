// Keyboard-first command palette (v0.4 Step 10). Fuzzy-searches named UI
// actions (workspace switches, run/undo/redo/etc — callbacks the caller
// already dispatches through the existing command paths) plus the
// journal-grammar verbs. Selecting a grammar verb either autocompletes it
// into the input (bare verb, needs args) or, once the line has arguments,
// dispatches it through `onExec` — which the caller wires to `console_exec`,
// the one grammar mutation path, so every palette dispatch is journaled
// exactly like typing it into the console. This component never calls a
// Tauri command directly.
import { useEffect, useMemo, useRef, useState } from "react";
import { GRAMMAR_COMMANDS } from "../core/grammarCommands";
import { filterPaletteItems, type PaletteItem } from "../core/paletteMatch";

export interface PaletteAction {
  id: string;
  title: string;
  keywords?: string;
  run: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  actions: PaletteAction[];
  onExec: (line: string) => Promise<void>;
}

type Item = PaletteItem & { run?: () => void };

export default function CommandPalette({ open, onClose, actions, onExec }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const items: Item[] = useMemo(
    () => [
      ...actions.map((a) => ({
        id: a.id,
        title: a.title,
        keywords: a.keywords,
        kind: "action" as const,
        run: a.run,
      })),
      ...GRAMMAR_COMMANDS.map((g) => ({
        id: `grammar:${g.verb}`,
        title: g.verb,
        subtitle: g.usage,
        keywords: g.description,
        kind: "grammar" as const,
        verb: g.verb,
      })),
    ],
    [actions],
  );

  // Once the query has a space, the user is past the verb and typing
  // arguments (e.g. "select-motor C6") — filter on the verb alone so the
  // matching entry stays visible instead of fuzzy-matching the whole line
  // (which would fail once argument text no longer subsequence-matches
  // the verb's title).
  const trimmedQuery = query.trim();
  const filterQuery = trimmedQuery.includes(" ") ? trimmedQuery.split(/\s+/)[0] : query;
  const filtered = useMemo(() => filterPaletteItems(items, filterQuery), [items, filterQuery]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setSelected(0);
    setError(null);
  }, [open]);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  if (!open) return null;

  const runItem = async (item: Item) => {
    if (item.kind === "action") {
      item.run?.();
      onClose();
      return;
    }
    // Grammar item: a query that is strictly the verb plus more tokens is
    // treated as a ready-to-dispatch command line. A bare verb (or a
    // prefix of it) is an autocomplete pick — fill it in and let the user
    // keep typing arguments.
    const trimmed = query.trim();
    const [firstToken, ...restTokens] = trimmed.split(/\s+/);
    const isReadyLine = firstToken === item.verb && restTokens.length > 0;
    if (isReadyLine) {
      try {
        await onExec(trimmed);
        onClose();
      } catch (e) {
        setError(String(e));
      }
      return;
    }
    setQuery(`${item.verb} `);
    setError(null);
    setSelected(0);
    inputRef.current?.focus();
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => Math.min(s + 1, Math.max(filtered.length - 1, 0)));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = filtered[selected];
      if (item) void runItem(item);
    }
  };

  return (
    <div className="command-palette-overlay" onClick={onClose}>
      <div className="command-palette" onClick={(e) => e.stopPropagation()}>
        <input
          ref={inputRef}
          className="command-palette-input"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder="Type a command or action…"
          aria-label="command palette input"
        />
        {error && <div className="command-palette-error">{error}</div>}
        <ul className="command-palette-list" role="listbox">
          {filtered.length === 0 && <li className="command-palette-empty">No matches</li>}
          {filtered.map((item, i) => (
            <li
              key={item.id}
              role="option"
              aria-selected={i === selected}
              className={`command-palette-item${i === selected ? " selected" : ""}`}
              onMouseEnter={() => setSelected(i)}
              onClick={() => void runItem(item)}
            >
              <span className="command-palette-item-title">{item.title}</span>
              {item.subtitle && (
                <span className="command-palette-item-subtitle">{item.subtitle}</span>
              )}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
