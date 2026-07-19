import { describe, expect, it } from "vitest";
import { filterPaletteItems, fuzzyScore, type PaletteItem } from "./paletteMatch";

describe("fuzzyScore", () => {
  it("matches an empty query against everything with score 0", () => {
    expect(fuzzyScore("", "add-part")).toBe(0);
    expect(fuzzyScore("   ", "add-part")).toBe(0);
  });

  it("matches a subsequence in order, case-insensitively", () => {
    expect(fuzzyScore("adpt", "add-part")).not.toBeNull();
    expect(fuzzyScore("ADPT", "add-part")).toBe(fuzzyScore("adpt", "add-part"));
  });

  it("rejects out-of-order or missing characters", () => {
    expect(fuzzyScore("tpa", "add-part")).toBeNull();
    expect(fuzzyScore("xyz", "add-part")).toBeNull();
    expect(fuzzyScore("add-part-extra", "add-part")).toBeNull();
  });

  it("scores a contiguous prefix match higher than a scattered one", () => {
    const contiguous = fuzzyScore("add", "add-part")!;
    const scattered = fuzzyScore("adt", "add-part")!;
    expect(contiguous).toBeGreaterThan(scattered);
  });

  it("prefers a shorter target among equal-quality matches", () => {
    const short = fuzzyScore("add", "add-part")!;
    const long = fuzzyScore("add", "add-part-with-a-much-longer-name")!;
    expect(short).toBeGreaterThan(long);
  });
});

describe("filterPaletteItems", () => {
  const items: PaletteItem[] = [
    { id: "1", title: "add-part", kind: "grammar", verb: "add-part", subtitle: "add-part <parent|-> <kind-json>" },
    { id: "2", title: "remove-part", kind: "grammar", verb: "remove-part" },
    { id: "3", title: "Switch to Design", kind: "action", keywords: "workspace tab design" },
    { id: "4", title: "Switch to Results", kind: "action", keywords: "workspace tab results" },
  ];

  it("returns everything, in original order, for an empty query", () => {
    expect(filterPaletteItems(items, "")).toEqual(items);
  });

  it("filters out non-matching items", () => {
    const result = filterPaletteItems(items, "design");
    expect(result.map((i) => i.id)).toEqual(["3"]);
  });

  it("matches keywords not shown in the title", () => {
    const result = filterPaletteItems(items, "tab");
    expect(result.map((i) => i.id).sort()).toEqual(["3", "4"]);
  });

  it("ranks the best match first when the query is ambiguous", () => {
    const result = filterPaletteItems(items, "part");
    // "add-part" and "remove-part" both match; "add-part" is shorter so it
    // should rank first (tie-break rule under equal-quality subsequence).
    expect(result[0].id).toBe("1");
  });

  it("returns an empty list when nothing matches", () => {
    expect(filterPaletteItems(items, "zzzzz")).toEqual([]);
  });

  it("matches the subtitle field too", () => {
    const result = filterPaletteItems(items, "kind-json");
    expect(result.map((i) => i.id)).toEqual(["1"]);
  });
});
