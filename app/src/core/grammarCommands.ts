// Static mirror of the journal-grammar verbs the Rust `Command::to_text` /
// `parse_text` pair accepts (crates/ascent-app/src/command.rs, spec in
// docs/JOURNAL_FORMAT.md). Rust owns parsing and validation; this list only
// drives the command palette's fuzzy search and autocomplete template —
// dispatch always goes through `console_exec`, which re-parses and
// validates server-side, so a stale or wrong entry here can only fail to
// suggest, never mis-dispatch.
export interface GrammarCommand {
  verb: string;
  usage: string;
  description: string;
}

export const GRAMMAR_COMMANDS: GrammarCommand[] = [
  {
    verb: "add-part",
    usage: "add-part <parent|-> <kind-json>",
    description: "Add a part to the vehicle tree",
  },
  {
    verb: "remove-part",
    usage: "remove-part <id>",
    description: "Remove a part by id",
  },
  {
    verb: "set-part-param",
    usage: "set-part-param <id> <param> <value-json>",
    description: "Set a part parameter",
  },
  {
    verb: "set-sim-param",
    usage: "set-sim-param <param> <value-json>",
    description: "Set a simulation parameter",
  },
  {
    verb: "select-motor",
    usage: "select-motor <designation>",
    description: "Select the motor",
  },
  {
    verb: "create-study",
    usage: 'create-study <"name"> <engine> <seed> <kind-json>',
    description: "Create a study",
  },
  {
    verb: "set-study-param",
    usage: "set-study-param <id> <param> <value-json>",
    description: "Set a study parameter",
  },
  {
    verb: "delete-study",
    usage: "delete-study <id>",
    description: "Delete a study",
  },
  {
    verb: "restore-part",
    usage: "restore-part <parent|-> <index> <part-json>",
    description: "Restore a removed part subtree at an index",
  },
  {
    verb: "set-design",
    usage: "set-design <design-json>",
    description: "Replace the whole design",
  },
  {
    verb: "restore-study",
    usage: "restore-study <index> <study-json>",
    description: "Restore a deleted study at an index",
  },
  {
    verb: "set-study-results",
    usage: "set-study-results <id> <results-json|null>",
    description: "Set or clear a study's results",
  },
];
