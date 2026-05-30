/**
 * tracker.js — OpenCode plugin that persists every event to events.jsonl
 *
 * Each line written to events.jsonl is a JSON object:
 * {
 *   "ts":        "<ISO timestamp>",
 *   "type":      "<event type string>",    // from Event union
 *   "hook":      "<hook name | null>",     // e.g. "tool.execute.before"
 *   "sessionID": "<session id | null>",
 *   "callID":    "<tool call id | null>",
 *   "tool":      "<tool name | null>",
 *   "args":      <tool args object | null>,
 *   "output":    <hook output snapshot | null>,
 *   "properties": <event properties | null>  // raw SDK event payload
 * }
 *
 * File location: <project root>/events.jsonl
 * Rotates at 50 MB to events.jsonl.1 (keeps 1 backup).
 */

import fs from "fs";
import path from "path";

const MAX_BYTES = 50 * 1024 * 1024; // 50 MB

/** Append one JSON line, rotating if needed */
function append(filePath, record) {
  const line = JSON.stringify(record) + "\n";

  // Rotate if file exceeds limit
  try {
    const stat = fs.statSync(filePath);
    if (stat.size + line.length > MAX_BYTES) {
      const backup = filePath + ".1";
      if (fs.existsSync(backup)) fs.unlinkSync(backup);
      fs.renameSync(filePath, backup);
    }
  } catch {
    // file doesn't exist yet — no rotation needed
  }

  fs.appendFileSync(filePath, line, "utf-8");
}

/** Build a base record with common fields */
function base(hook, sessionID, callID, tool) {
  return {
    ts: new Date().toISOString(),
    type: null,
    hook,
    sessionID: sessionID ?? null,
    callID: callID ?? null,
    tool: tool ?? null,
    args: null,
    output: null,
    properties: null,
  };
}

export const TrackerPlugin = async ({ directory }) => {
  const eventsFile = path.join(directory, "events.jsonl");

  // Write a startup marker
  append(eventsFile, {
    ...base("plugin.init", null, null, null),
    type: "plugin.init",
    properties: { directory },
  });

  return {
    // ── Raw SDK event stream ──────────────────────────────────────────────
    event: async ({ event }) => {
      if (!event.type.startWith("file.watcher")) {
        append(eventsFile, {
          ts: new Date().toISOString(),
          ...event,
        });
      }
    },

    // // ── Tool lifecycle ────────────────────────────────────────────────────
    "tool.execute.before": async (input, output) => {
      append(eventsFile, {
        ...base(
          "tool.execute.before",
          input.sessionID,
          input.callID,
          input.tool,
        ),
        type: "tool.execute.before",
        args: output.args ?? null,
      });
      output.args.sessionId = input.sessionID;
    },

    "tool.execute.after": async (input, output) => {
      append(eventsFile, {
        ...base(
          "tool.execute.after",
          input.sessionID,
          input.callID,
          input.tool,
        ),
        type: "tool.execute.after",
        args: input.args ?? null,
        output: {
          title: output.title ?? null,
          output: output.output ?? null,
          metadata: output.metadata ?? null,
        },
      });
      if (input.args.sessionId) {
        delete input.args.sessionId;
      }
    },

    // // ── Chat lifecycle ────────────────────────────────────────────────────
    // "chat.message": async (input, output) => {
    //   append(eventsFile, {
    //     ...base("chat.message", input.sessionID, null, null),
    //     type: "chat.message",
    //     properties: {
    //       agent: input.agent ?? null,
    //       model: input.model ?? null,
    //       messageID: input.messageID ?? null,
    //       variant: input.variant ?? null,
    //       partCount: output.parts?.length ?? 0,
    //     },
    //   });
    // },

    // "chat.params": async (input, _output) => {
    //   append(eventsFile, {
    //     ...base("chat.params", input.sessionID, null, null),
    //     type: "chat.params",
    //     properties: {
    //       agent: input.agent,
    //       model: {
    //         providerID: input.model?.providerID,
    //         modelID: input.model?.modelID,
    //       },
    //     },
    //   });
    // },

    // // ── Permission ────────────────────────────────────────────────────────
    // "permission.ask": async (input, output) => {
    //   append(eventsFile, {
    //     ...base("permission.ask", null, null, null),
    //     type: "permission.ask",
    //     properties: { permission: input, decision: output.status },
    //   });
    // },

    // // ── Command ───────────────────────────────────────────────────────────
    // "command.execute.before": async (input, _output) => {
    //   append(eventsFile, {
    //     ...base("command.execute.before", input.sessionID, null, null),
    //     type: "command.execute.before",
    //     properties: { command: input.command, arguments: input.arguments },
    //   });
    // },

    // // ── Compaction ────────────────────────────────────────────────────────
    // "experimental.session.compacting": async (input, _output) => {
    //   append(eventsFile, {
    //     ...base("experimental.session.compacting", input.sessionID, null, null),
    //     type: "experimental.session.compacting",
    //   });
    // },
  };
};
