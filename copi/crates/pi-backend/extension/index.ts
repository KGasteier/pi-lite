/**
 * agentscope – pi-Erweiterung, die die Aktivität dieses pi-Prozesses (Turns, Tools,
 * Subagents, Tokens, Kontext, Kompaktierung, Fehler) als JSON-Schnappschuss nach
 * $XDG_RUNTIME_DIR/agentscope/<pid>.json schreibt. Der Screensaver liest diese Dateien.
 *
 * Läuft in allen Modi (tui, rpc, json/print). Subagent-Kindprozesse (pi --mode json)
 * laden die Erweiterung ebenfalls und tragen über `ppid` ihre Herkunft ein.
 *
 * Für entfernte Backends (copi/cocopi über SSH) setzt `AGENTSCOPE_DIR` ein festes, gut
 * auffindbares Ziel: dort fehlt `XDG_RUNTIME_DIR` in nackten SSH-Sitzungen, und `/tmp`
 * wäre nicht stabil genug zum Spiegeln.
 *
 * Abschalten: AGENTSCOPE=off.
 */

import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { stateDir, Tracker } from "./core.ts";

const DIR = stateDir(process.env, os.tmpdir());
const FILE = path.join(DIR, `${process.pid}.json`);
const MIN_INTERVAL_MS = 250;
const HEARTBEAT_MS = 2000;

export default function agentscope(pi: ExtensionAPI) {
	if (process.env.AGENTSCOPE === "off") return;

	let tracker: Tracker | null = null;
	let lastWrite = 0;
	let pending: NodeJS.Timeout | null = null;
	let heartbeat: NodeJS.Timeout | null = null;
	// biome-ignore lint/suspicious/noExplicitAny: Kontexttyp je nach pi-Version verschieden
	let lastCtx: any = null;

	const write = () => {
		pending = null;
		if (!tracker) return;
		const now = Date.now();
		try {
			if (lastCtx) tracker.setContext(lastCtx.getContextUsage?.(), now);
		} catch {
			/* Kontext evtl. ungültig nach Sessionwechsel */
		}
		lastWrite = now;
		try {
			fs.mkdirSync(DIR, { recursive: true, mode: 0o700 });
			const tmp = `${FILE}.tmp`;
			fs.writeFileSync(tmp, JSON.stringify(tracker.snapshot(now)));
			fs.renameSync(tmp, FILE);
		} catch {
			/* Anzeige ist optional – niemals pi stören */
		}
	};

	const flush = () => {
		if (pending) return;
		const wait = Math.max(0, MIN_INTERVAL_MS - (Date.now() - lastWrite));
		pending = setTimeout(write, wait);
		pending.unref?.();
	};

	// biome-ignore lint/suspicious/noExplicitAny: siehe oben
	const on = (name: string, fn: (e: any, ctx: any, now: number) => void) => {
		// biome-ignore lint/suspicious/noExplicitAny: Ereignisnamen variieren zwischen Versionen
		(pi as any).on(name, (e: any, ctx: any) => {
			try {
				if (!tracker) return;
				if (ctx) lastCtx = ctx;
				fn(e, ctx, Date.now());
				flush();
			} catch {
				/* nie den Agenten stören */
			}
		});
	};

	// biome-ignore lint/suspicious/noExplicitAny: siehe oben
	(pi as any).on("session_start", (_e: any, ctx: any) => {
		try {
			tracker = new Tracker({ pid: process.pid, ppid: process.ppid, cwd: ctx?.cwd ?? process.cwd(), now: Date.now() });
			lastCtx = ctx;
			const sm = ctx?.sessionManager;
			tracker.setMeta(
				{
					sessionFile: sm?.getSessionFile?.() ?? null,
					sessionName: sm?.getSessionName?.() ?? null,
					model: ctx?.model ? `${ctx.model.provider}/${ctx.model.id}` : null,
				},
				Date.now(),
			);
			if (!heartbeat) {
				heartbeat = setInterval(flush, HEARTBEAT_MS);
				heartbeat.unref?.();
			}
			write();
		} catch {
			/* ignorieren */
		}
	});

	on("input", (e, _c, now) => tracker!.input(e?.text ?? "", now));
	on("agent_start", (_e, _c, now) => tracker!.agentStart(now));
	on("turn_start", (e, _c, now) => tracker!.turnStart(e?.turnIndex, now));
	on("message_update", (e, _c, now) => {
		const ev = e?.assistantMessageEvent;
		if (ev && (ev.type === "text_delta" || ev.type === "thinking_delta" || ev.type === "toolcall_delta")) {
			tracker!.streamDelta(ev.type, ev.delta ?? "", now);
		}
	});
	on("message_end", (e, ctx, now) => {
		if (e?.message?.role === "assistant") tracker!.assistantEnd(e.message, now);
		if (ctx?.model) tracker!.setMeta({ model: `${ctx.model.provider}/${ctx.model.id}` }, now);
	});
	on("tool_execution_start", (e, _c, now) => tracker!.toolStart(e.toolCallId, e.toolName, e.args, now));
	on("tool_execution_update", (e, _c, now) => tracker!.toolUpdate(e.toolCallId, e.toolName, e.partialResult, now));
	on("tool_execution_end", (e, _c, now) => tracker!.toolEnd(e.toolCallId, e.toolName, e.result, !!e.isError, now));
	on("session_before_compact", (e, _c, now) =>
		tracker!.compactStart(e?.reason ?? "threshold", e?.preparation?.tokensBefore ?? null, now),
	);
	on("session_compact", (e, _c, now) => tracker!.compactEnd(e?.compactionEntry, now));
	on("session_compact_failed", (e, _c, now) => {
		if (e?.aborted) {
			tracker!.state.compaction.active = false;
			tracker!.event("compact_failed", "Compaction abgebrochen", now);
		} else tracker!.compactFailed(e?.errorMessage ?? "unknown", now);
	});
	on("auto_retry_start", (e, _c, now) => tracker!.retryStart(e?.attempt ?? 1, e?.maxAttempts ?? 3, e?.errorMessage ?? "", now));
	on("auto_retry_end", (e, _c, now) => tracker!.retryEnd(!!e?.success, now));
	on("agent_settled", (_e, _c, now) => tracker!.settled(now));
	on("session_info_changed", (e, _c, now) => tracker!.setMeta({ sessionName: e?.name ?? null }, now));

	// biome-ignore lint/suspicious/noExplicitAny: siehe oben
	(pi as any).on("session_shutdown", () => {
		if (pending) clearTimeout(pending);
		if (heartbeat) clearInterval(heartbeat);
		pending = null;
		heartbeat = null;
		tracker = null;
		try {
			fs.unlinkSync(FILE);
		} catch {
			/* schon weg */
		}
	});
}
