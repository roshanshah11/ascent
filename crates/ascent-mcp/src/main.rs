//! ascent-mcp: MCP stdio server for the Ascent copilot seam.
//! Wire an MCP client at this binary; stdin/stdout is the whole transport.

fn main() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    ascent_mcp::serve(stdin.lock(), stdout.lock())
}
