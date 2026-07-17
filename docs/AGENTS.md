<!-- headroom:rtk-instructions -->
# RTK (Rust Token Killer) - Token-Optimized Commands

When running shell commands, **always prefix with `rtk`**. This reduces context
usage by 60-90% with zero behavior change. Most unfiltered commands pass through
unchanged, but see Known Limits below — some commands must NOT be prefixed.

## Key Commands
```bash
# Git (59-80% savings)
rtk git status          rtk git diff            rtk git log

# Files & Search (60-75% savings)
rtk ls <path>           rtk read <file>         rtk grep <pattern>
rtk find <pattern>      rtk diff <file>

# Test (90-99% savings) — shows failures only
rtk pytest tests/       rtk cargo test          rtk test <cmd>

# Build & Lint (80-90% savings) — shows errors only
rtk tsc                 rtk lint                rtk cargo build
rtk prettier --check    rtk mypy                rtk ruff check

# Analysis (70-90% savings)
rtk err <cmd>           rtk log <file>          rtk json <file>
rtk summary <cmd>       rtk deps                rtk env

# GitHub (26-87% savings)
rtk gh pr view <n>      rtk gh run list         rtk gh issue list

# Infrastructure (85% savings)
rtk docker ps           rtk kubectl get         rtk docker logs <c>

# Package managers (70-90% savings)
rtk pip list            rtk pnpm install        rtk npm run <script>
```

## Rules
- In command chains, prefix each segment: `rtk git add . && rtk git commit -m "msg"`
- For debugging, use raw command without rtk prefix
- `rtk proxy <cmd>` runs command without filtering but tracks usage

## Known Limits (do NOT retry rtk variants on these errors)
- `find` with `-o`/`-not`/`-exec`/parens: rtk find rejects compound predicates — use plain `find` (or `/usr/bin/find`) directly
- `npx`: never `rtk npx` (gets mangled to `npm run`) — use `rtk proxy npx …`
- `tail` and multi-file `cat`: use raw commands (rtk read is single-file, no tail support)
- If rtk errors on any command, fall back to the raw command immediately — do not loop on rtk retries
<!-- /headroom:rtk-instructions -->
