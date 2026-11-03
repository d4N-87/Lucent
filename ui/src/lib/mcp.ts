import { listen } from "@tauri-apps/api/event";
import { mcpReply } from "./api";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type ToolHandler = (args: any) => unknown | Promise<unknown>;

/**
 * Answers tool calls forwarded by the MCP server (see src-tauri/src/mcp.rs).
 * A thrown Error becomes a tool error whose message the agent reads, so messages
 * should say how to fix the call.
 */
export function serveMcp(handlers: Record<string, ToolHandler>, oncall: (tool: string) => void) {
  return listen<{ id: number; tool: string; args: unknown }>("mcp-call", async ({ payload }) => {
    const { id, tool, args } = payload;
    try {
      const handler = handlers[tool];
      if (!handler) throw new Error(`Unknown tool "${tool}"`);
      oncall(tool);
      const result = await handler(args ?? {});
      await mcpReply(id, result ?? { ok: true }, null);
    } catch (e) {
      await mcpReply(id, null, e instanceof Error ? e.message : String(e));
    }
  });
}
