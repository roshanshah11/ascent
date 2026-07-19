// Pure fuzzy matching for the command palette (v0.4 Step 10). No DOM, no
// IPC — subsequence match over an item's searchable text with a score that
// rewards prefix hits and contiguous runs, the classic Cmd+K feel. Filtering
// and dispatch decisions live in the component; this module only ranks.

export interface PaletteItem {
  id: string;
  /** Primary label shown in the list and matched against the query. */
  title: string;
  /** Secondary text shown dimmed (e.g. grammar usage line). */
  subtitle?: string;
  /** Extra searchable text not shown (aliases, keywords). */
  keywords?: string;
  kind: "action" | "grammar";
  /** Present when kind === "grammar": the canonical verb, e.g. "add-part". */
  verb?: string;
}

/**
 * Subsequence fuzzy score: every character of `query` (case-insensitive)
 * must appear in `target` in order, not necessarily contiguous. Returns
 * `null` when no match. Higher score = better match. An empty query
 * matches everything with score 0 (palette shows the full list on open).
 */
export function fuzzyScore(query: string, target: string): number | null {
  const q = query.trim().toLowerCase();
  if (q.length === 0) return 0;
  const t = target.toLowerCase();

  let score = 0;
  let tIndex = 0;
  let prevMatchIndex = -1;
  for (let qIndex = 0; qIndex < q.length; qIndex += 1) {
    const ch = q[qIndex];
    const found = t.indexOf(ch, tIndex);
    if (found === -1) return null;
    // Contiguous run bonus: matching right after the previous match beats
    // a scattered match.
    score += found === prevMatchIndex + 1 ? 5 : 1;
    // Prefix bonus: matching at the very start of the target is the
    // strongest signal (typing "add" should put "add-part" on top).
    if (found === 0) score += 3;
    prevMatchIndex = found;
    tIndex = found + 1;
  }
  // Shorter targets rank slightly higher among equal-quality matches —
  // "add-part" over "restore-part" when both match "add".
  score -= target.length * 0.01;
  return score;
}

function searchableText(item: PaletteItem): string[] {
  const fields = [item.title];
  if (item.subtitle) fields.push(item.subtitle);
  if (item.keywords) fields.push(item.keywords);
  return fields;
}

/**
 * Filters and ranks items against a query. Each item is scored by its
 * best-matching field; items with no matching field are dropped. Ties keep
 * the original (stable) order, so callers control default ordering by the
 * order they pass items in.
 */
export function filterPaletteItems(items: PaletteItem[], query: string): PaletteItem[] {
  const scored: { item: PaletteItem; score: number; index: number }[] = [];
  items.forEach((item, index) => {
    let best: number | null = null;
    for (const field of searchableText(item)) {
      const s = fuzzyScore(query, field);
      if (s !== null && (best === null || s > best)) best = s;
    }
    if (best !== null) scored.push({ item, score: best, index });
  });
  scored.sort((a, b) => b.score - a.score || a.index - b.index);
  return scored.map((s) => s.item);
}
