#!/usr/bin/env node
/**
 * track — tail and pretty-print events.jsonl written by the tracker plugin
 *
 * Usage:
 *   node .opencode/track.js              # tail live
 *   node .opencode/track.js --all        # dump all past events then tail
 *   node .opencode/track.js --filter session   # only lines whose type contains "session"
 *   node .opencode/track.js --json       # raw JSON output (no formatting)
 *   node .opencode/track.js --no-tail    # print existing events and exit
 */

import fs from "fs"
import path from "path"
import readline from "readline"
import { parseArgs } from "util"

// ── CLI args ──────────────────────────────────────────────────────────────────
const { values: flags } = parseArgs({
  options: {
    all:     { type: "boolean", default: false },
    filter:  { type: "string",  default: "" },
    json:    { type: "boolean", default: false },
    "no-tail": { type: "boolean", default: false },
    help:    { type: "boolean", default: false },
  },
  allowPositionals: true,
  strict: false,
})

if (flags.help) {
  console.log(`
track — tail events.jsonl from the OpenCode tracker plugin

  --all          Dump all existing events before tailing
  --filter <str> Only show events whose type contains <str>
  --json         Raw JSON output (no color/formatting)
  --no-tail      Print existing events and exit (no live tail)
  --help         This help
`)
  process.exit(0)
}

// ── Resolve events file ───────────────────────────────────────────────────────
const cwd = process.cwd()
const eventsFile = path.join(cwd, "events.jsonl")

if (!fs.existsSync(eventsFile)) {
  console.error(`events.jsonl not found at: ${eventsFile}`)
  console.error("Start opencode with the tracker plugin enabled to generate events.")
  process.exit(1)
}

// ── Colors ────────────────────────────────────────────────────────────────────
const c = {
  reset:  "\x1b[0m",
  dim:    "\x1b[2m",
  bold:   "\x1b[1m",
  cyan:   "\x1b[36m",
  green:  "\x1b[32m",
  yellow: "\x1b[33m",
  red:    "\x1b[31m",
  magenta:"\x1b[35m",
  blue:   "\x1b[34m",
  white:  "\x1b[37m",
}

const noColor = !process.stdout.isTTY || flags.json
const col = (color, str) => noColor ? str : `${color}${str}${c.reset}`

// ── Type → color mapping ──────────────────────────────────────────────────────
function typeColor(type) {
  if (!type) return c.dim
  if (type.startsWith("tool.execute.before"))  return c.yellow
  if (type.startsWith("tool.execute.after"))   return c.green
  if (type.startsWith("chat."))                return c.cyan
  if (type.startsWith("session."))             return c.blue
  if (type.startsWith("permission."))          return c.red
  if (type.startsWith("command."))             return c.magenta
  if (type.startsWith("file."))                return c.white
  if (type.startsWith("message."))             return c.white
  if (type.startsWith("experimental."))        return c.dim
  if (type === "plugin.init")                  return c.bold
  return c.dim
}

// ── Format one parsed record ──────────────────────────────────────────────────
function format(record) {
  if (flags.json) return JSON.stringify(record)

  const time = col(c.dim, record.ts ? record.ts.slice(11, 23) : "??:??:??.???")
  const type = col(typeColor(record.type), (record.type ?? "unknown").padEnd(35))
  const hook = col(c.dim, (record.hook ?? "").padEnd(25))

  const parts = [time, type]

  if (record.sessionID) parts.push(col(c.dim, `sid:${record.sessionID.slice(0, 8)}`))
  if (record.tool)      parts.push(col(c.cyan, `tool:${record.tool}`))
  if (record.callID)    parts.push(col(c.dim, `call:${record.callID.slice(0, 8)}`))

  // Summarize args / output / properties
  if (record.args && Object.keys(record.args).length > 0) {
    const summary = summarize(record.args)
    parts.push(col(c.yellow, `args:${summary}`))
  }

  if (record.output?.output) {
    const out = String(record.output.output).slice(0, 80).replace(/\n/g, "↵")
    parts.push(col(c.green, `out:${out}`))
  }

  if (record.properties) {
    const summary = summarize(record.properties)
    if (summary) parts.push(col(c.dim, summary))
  }

  return parts.join("  ")
}

function summarize(obj) {
  if (!obj || typeof obj !== "object") return String(obj ?? "")
  const keys = Object.keys(obj)
  if (keys.length === 0) return ""
  return keys.slice(0, 3).map(k => {
    const v = obj[k]
    if (v == null) return null
    if (typeof v === "object") return `${k}:{…}`
    const s = String(v).slice(0, 40)
    return `${k}:${s}`
  }).filter(Boolean).join(" ")
}

// ── Filter ────────────────────────────────────────────────────────────────────
const filterStr = flags.filter?.toLowerCase() ?? ""

function shouldShow(record) {
  if (!filterStr) return true
  return (record.type ?? "").toLowerCase().includes(filterStr) ||
         (record.hook ?? "").toLowerCase().includes(filterStr) ||
         (record.tool ?? "").toLowerCase().includes(filterStr)
}

// ── Print one line ────────────────────────────────────────────────────────────
function printLine(raw) {
  raw = raw.trim()
  if (!raw) return
  let record
  try {
    record = JSON.parse(raw)
  } catch {
    console.log(col(c.red, `[parse error] ${raw.slice(0, 120)}`))
    return
  }
  if (!shouldShow(record)) return
  console.log(format(record))
}

// ── Dump existing content ─────────────────────────────────────────────────────
let startOffset = 0

if (flags.all || flags["no-tail"]) {
  const content = fs.readFileSync(eventsFile, "utf-8")
  const lines = content.split("\n")
  for (const line of lines) printLine(line)
  startOffset = Buffer.byteLength(content, "utf-8")
} else {
  // Default: start from end of file (only new events)
  try {
    startOffset = fs.statSync(eventsFile).size
  } catch {
    startOffset = 0
  }
}

if (flags["no-tail"]) process.exit(0)

// ── Tail new lines ────────────────────────────────────────────────────────────
if (!flags["no-tail"]) {
  console.log(col(c.dim, `--- tailing ${eventsFile} (Ctrl+C to stop) ---`))
}

let fileOffset = startOffset
let buffer = ""

const watcher = fs.watch(eventsFile, { persistent: true }, (eventType) => {
  if (eventType !== "change") return
  try {
    const stat = fs.statSync(eventsFile)
    // File was rotated (truncated/replaced) — reset
    if (stat.size < fileOffset) {
      fileOffset = 0
      buffer = ""
    }

    if (stat.size <= fileOffset) return

    const fd = fs.openSync(eventsFile, "r")
    const chunkSize = stat.size - fileOffset
    const buf = Buffer.alloc(chunkSize)
    fs.readSync(fd, buf, 0, chunkSize, fileOffset)
    fs.closeSync(fd)

    fileOffset = stat.size
    buffer += buf.toString("utf-8")

    const lines = buffer.split("\n")
    buffer = lines.pop() ?? "" // keep incomplete last line
    for (const line of lines) printLine(line)
  } catch {
    // ignore transient errors during rotation
  }
})

process.on("SIGINT", () => {
  watcher.close()
  process.exit(0)
})
