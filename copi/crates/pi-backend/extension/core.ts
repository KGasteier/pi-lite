/**
 * agentscope core – reine Zustandslogik ohne pi-Abhängigkeit (mit `node --test` prüfbar).
 *
 * Ein Tracker sammelt Ereignisse eines pi-Prozesses und liefert einen Schnappschuss
 * (Schema v1, siehe docs/STATE.md), den die Erweiterung atomar als JSON ablegt.
 */

import { isAbsolute, join, resolve } from "node:path";

export type Phase = "idle" | "thinking" | "streaming" | "tool" | "compacting" | "retrying" | "error";

export interface ScopeEvent {
	seq: number;
	at: number;
	kind: string; // tool | tool_error | error | compact | compact_failed | retry | input | subagent | done
	text: string;
}

export interface SubAgentState {
	agent: string;
	task: string;
	status: "running" | "done" | "failed";
	turns: number;
	tokens: number;
	contextTokens: number;
	lastTool: string;
	error: string;
}

export interface SubagentRun {
	toolCallId: string;
	mode: string;
	total: number;
	done: number;
	running: number;
	failed: number;
	finished: boolean;
	agents: SubAgentState[];
}

export interface ScopeState {
	v: 1;
	pid: number;
	ppid: number;
	started: number;
	updated: number;
	cwd: string;
	sessionFile: string | null;
	sessionName: string | null;
	model: string | null;
	phase: Phase;
	prompt: string;
	turn: number;
	context: { tokens: number | null; window: number | null; percent: number | null };
	tokens: { input: number; output: number; cacheRead: number; total: number; rateOut: number };
	tools: { id: string; name: string; label: string; started: number }[];
	toolCounts: Record<string, number>;
	errors: number;
	lastError: { msg: string; at: number } | null;
	compaction: {
		active: boolean;
		reason: string;
		tokensBefore: number | null;
		tokensAfter: number | null;
		at: number;
		count: number;
	};
	retry: { attempt: number; max: number; error: string } | null;
	subagentRuns: SubagentRun[];
	events: ScopeEvent[];
}

const MAX_EVENTS = 40;
const MAX_RUNS = 4;
const RATE_WINDOW_MS = 4000;

/**
 * Zielverzeichnis der Zustandsdateien. `AGENTSCOPE_DIR` gewinnt und ist der Haken für entfernte
 * Backends (copi/cocopi über SSH): dort fehlt `XDG_RUNTIME_DIR` in nackten SSH-Sitzungen, und
 * `/tmp` ist zu unbeständig zum Spiegeln. Danach das übliche Laufzeitverzeichnis, sonst `tmp`.
 */
export function stateDir(env: Record<string, string | undefined>, tmp: string): string {
	const override = (env.AGENTSCOPE_DIR ?? "").trim();
	if (override) return isAbsolute(override) ? override : resolve(override);
	return join(env.XDG_RUNTIME_DIR ?? tmp, "agentscope");
}

export function clip(s: unknown, n: number): string {
	const t = String(s ?? "").replace(/\s+/g, " ").trim();
	return t.length > n ? `${t.slice(0, n - 1)}…` : t;
}

/** Kurzlabel eines Tool-Aufrufs, z. B. `bash: cargo test` oder `read: main.rs`. */
export function toolLabel(name: string, args: any): string {
	const a = args ?? {};
	const base = (p: unknown) => String(p ?? "").split("/").filter(Boolean).pop() ?? "";
	switch (name) {
		case "bash":
			return clip(a.command, 48);
		case "read":
		case "edit":
		case "write":
			return base(a.path ?? a.file_path);
		case "subagent": {
			if (Array.isArray(a.tasks) && a.tasks.length) return `${a.tasks.length} parallel`;
			if (Array.isArray(a.chain) && a.chain.length) return `chain of ${a.chain.length}`;
			return clip(a.agent ?? "", 24);
		}
		default:
			return clip(JSON.stringify(a), 40);
	}
}

function lastToolOf(messages: any[] | undefined): string {
	if (!Array.isArray(messages)) return "";
	for (let i = messages.length - 1; i >= 0; i--) {
		const m = messages[i];
		if (m?.role === "assistant" && Array.isArray(m.content)) {
			for (let j = m.content.length - 1; j >= 0; j--) {
				const p = m.content[j];
				if (p?.type === "toolCall") return `${p.name}: ${toolLabel(p.name, p.arguments)}`;
			}
		}
	}
	return "";
}

/**
 * Die subagent-Erweiterung setzt exitCode erst am Ende; während des Laufs steht dort 0.
 * Ein Teilergebnis gilt deshalb erst als fertig, wenn die letzte Nachricht eine
 * abgeschlossene Assistent-Antwort ohne weiteren Tool-Aufruf ist.
 */
export function looksFinished(messages: any[] | undefined): boolean {
	if (!Array.isArray(messages) || !messages.length) return false;
	const m = messages[messages.length - 1];
	return m?.role === "assistant" && !!m.stopReason && m.stopReason !== "toolUse";
}

/** Wandelt `details` des subagent-Werkzeugs (partial oder final) in einen SubagentRun. */
export function parseSubagentDetails(toolCallId: string, details: any, finished: boolean, expected = 0): SubagentRun | null {
	if (!details || !Array.isArray(details.results)) return null;
	const agents: SubAgentState[] = details.results.map((r: any) => {
		const u = r.usage ?? {};
		const failed = r.exitCode > 0 || r.stopReason === "error" || r.stopReason === "aborted" || !!r.errorMessage;
		const running = !finished && !failed && (r.exitCode === -1 || r.exitCode === undefined || !looksFinished(r.messages));
		return {
			agent: clip(r.agent, 24),
			task: clip(r.task, 90),
			status: running ? "running" : failed ? "failed" : "done",
			turns: u.turns ?? 0,
			tokens: (u.input ?? 0) + (u.output ?? 0),
			contextTokens: u.contextTokens ?? 0,
			lastTool: clip(lastToolOf(r.messages), 60),
			error: clip(r.errorMessage ?? (failed ? r.stderr : ""), 80),
		} satisfies SubAgentState;
	});
	// Single/Chain-Modus melden nur das laufende Ergebnis – erwartete Anzahl aus den Argumenten.
	const total = Math.max(agents.length, expected);
	const done = agents.filter((a) => a.status === "done").length;
	const failed = agents.filter((a) => a.status === "failed").length;
	const running = agents.filter((a) => a.status === "running").length;
	return { toolCallId, mode: String(details.mode ?? "single"), total, done, running, failed, finished, agents };
}

export function expectedSubagents(args: any): number {
	if (Array.isArray(args?.tasks) && args.tasks.length) return args.tasks.length;
	if (Array.isArray(args?.chain) && args.chain.length) return args.chain.length;
	return 1;
}

export class Tracker {
	state: ScopeState;
	private seq = 0;
	private outSamples: { at: number; tok: number }[] = [];
	private subArgs = new Map<string, number>();

	constructor(init: { pid: number; ppid: number; cwd: string; now: number }) {
		this.state = {
			v: 1,
			pid: init.pid,
			ppid: init.ppid,
			started: init.now,
			updated: init.now,
			cwd: init.cwd,
			sessionFile: null,
			sessionName: null,
			model: null,
			phase: "idle",
			prompt: "",
			turn: 0,
			context: { tokens: null, window: null, percent: null },
			tokens: { input: 0, output: 0, cacheRead: 0, total: 0, rateOut: 0 },
			tools: [],
			toolCounts: {},
			errors: 0,
			lastError: null,
			compaction: { active: false, reason: "", tokensBefore: null, tokensAfter: null, at: 0, count: 0 },
			retry: null,
			subagentRuns: [],
			events: [],
		};
	}

	event(kind: string, text: string, now: number): void {
		this.state.events.push({ seq: ++this.seq, at: now, kind, text: clip(text, 90) });
		if (this.state.events.length > MAX_EVENTS) this.state.events.splice(0, this.state.events.length - MAX_EVENTS);
	}

	private touch(now: number) {
		this.state.updated = now;
	}

	private recomputePhase() {
		const s = this.state;
		if (s.compaction.active) s.phase = "compacting";
		else if (s.retry) s.phase = "retrying";
		else if (s.tools.length) s.phase = "tool";
	}

	setMeta(meta: Partial<Pick<ScopeState, "sessionFile" | "sessionName" | "model" | "cwd">>, now: number) {
		Object.assign(this.state, meta);
		this.touch(now);
	}

	setContext(u: { tokens?: number | null; contextWindow?: number | null; percent?: number | null } | undefined, now: number) {
		if (!u) return;
		this.state.context = {
			tokens: u.tokens ?? null,
			window: u.contextWindow ?? this.state.context.window,
			percent: u.percent ?? null,
		};
		this.touch(now);
	}

	input(text: string, now: number) {
		if (!text?.trim()) return;
		this.state.prompt = clip(text, 160);
		this.event("input", text, now);
		this.touch(now);
	}

	agentStart(now: number) {
		this.state.phase = "thinking";
		this.state.retry = null;
		this.touch(now);
	}

	turnStart(turn: number | undefined, now: number) {
		this.state.turn = turn ?? this.state.turn + 1;
		if (!this.state.tools.length) this.state.phase = "thinking";
		this.touch(now);
	}

	/** Streaming-Delta: Tokenrate wird aus Zeichen/4 geschätzt, falls der Provider keine Usage streamt. */
	streamDelta(kind: string, delta: string, now: number) {
		const tok = Math.max(1, Math.round((delta?.length ?? 0) / 4));
		this.outSamples.push({ at: now, tok });
		this.state.phase = kind === "thinking_delta" ? "thinking" : "streaming";
		this.recomputePhase();
		this.touch(now);
	}

	/** Gleitende Ausgaberate in Token/s über die letzten Sekunden. */
	rate(now: number): number {
		this.outSamples = this.outSamples.filter((s) => now - s.at <= RATE_WINDOW_MS);
		const sum = this.outSamples.reduce((a, s) => a + s.tok, 0);
		return Math.round((sum * 1000) / RATE_WINDOW_MS);
	}

	assistantEnd(msg: any, now: number) {
		const u = msg?.usage;
		if (u) {
			this.state.tokens.input += u.input ?? 0;
			this.state.tokens.output += u.output ?? 0;
			this.state.tokens.cacheRead += u.cacheRead ?? 0;
			this.state.tokens.total += u.totalTokens ?? (u.input ?? 0) + (u.output ?? 0);
		}
		if (msg?.model) this.state.model = String(msg.model);
		if (msg?.stopReason === "error" || msg?.errorMessage) {
			this.error(msg.errorMessage ?? "model error", now);
		}
		this.touch(now);
	}

	error(msg: string, now: number) {
		this.state.errors++;
		this.state.lastError = { msg: clip(msg, 120), at: now };
		this.state.phase = "error";
		this.event("error", msg, now);
		this.touch(now);
	}

	toolStart(id: string, name: string, args: any, now: number) {
		const label = toolLabel(name, args);
		this.state.tools.push({ id, name, label, started: now });
		this.state.toolCounts[name] = (this.state.toolCounts[name] ?? 0) + 1;
		if (name === "subagent") {
			const n = expectedSubagents(args);
			this.subArgs.set(id, n);
			this.upsertRun({ toolCallId: id, mode: args?.tasks?.length ? "parallel" : args?.chain?.length ? "chain" : "single", total: n, done: 0, running: 0, failed: 0, finished: false, agents: [] });
			this.event("subagent", `delegiert ${label}`, now);
		} else {
			this.event("tool", `${name}: ${label}`, now);
		}
		this.state.phase = "tool";
		this.recomputePhase();
		this.touch(now);
	}

	toolUpdate(id: string, name: string, partial: any, now: number) {
		if (name === "subagent") {
			const run = parseSubagentDetails(id, partial?.details, false, this.subArgs.get(id) ?? 0);
			if (run) this.upsertRun(run);
		}
		this.touch(now);
	}

	toolEnd(id: string, name: string, result: any, isError: boolean, now: number) {
		this.state.tools = this.state.tools.filter((t) => t.id !== id);
		if (name === "subagent") {
			const run = parseSubagentDetails(id, result?.details, true, this.subArgs.get(id) ?? 0);
			if (run) {
				// Bei Abschluss zählen nicht mehr gemeldete Agenten als fehlgeschlagen.
				this.upsertRun(run);
				this.event("done", `Subagents: ${run.done}/${run.total} fertig, ${run.failed} fehlgeschlagen`, now);
			} else {
				const r = this.state.subagentRuns.find((x) => x.toolCallId === id);
				if (r) r.finished = true;
			}
			this.subArgs.delete(id);
		}
		if (isError) {
			this.state.errors++;
			const text = Array.isArray(result?.content) ? result.content.map((c: any) => c?.text ?? "").join(" ") : "";
			this.state.lastError = { msg: clip(`${name}: ${text || "failed"}`, 120), at: now };
			this.event("tool_error", `${name}-Fehler: ${text}`, now);
		}
		if (!this.state.tools.length && this.state.phase === "tool") this.state.phase = "thinking";
		this.recomputePhase();
		this.touch(now);
	}

	private upsertRun(run: SubagentRun) {
		const runs = this.state.subagentRuns;
		const i = runs.findIndex((r) => r.toolCallId === run.toolCallId);
		if (i >= 0) runs[i] = run;
		else runs.push(run);
		if (runs.length > MAX_RUNS) runs.splice(0, runs.length - MAX_RUNS);
	}

	compactStart(reason: string, tokensBefore: number | null, now: number) {
		this.state.compaction = { ...this.state.compaction, active: true, reason, tokensBefore, tokensAfter: null, at: now };
		this.state.phase = "compacting";
		this.event("compact", `komprimiere Kontext (${reason})${tokensBefore ? ` ${Math.round(tokensBefore / 1000)}k` : ""}`, now);
		this.touch(now);
	}

	compactEnd(entry: any, now: number) {
		const c = this.state.compaction;
		c.active = false;
		c.count++;
		c.at = now;
		c.tokensBefore = entry?.tokensBefore ?? c.tokensBefore;
		c.tokensAfter = entry?.estimatedTokensAfter ?? entry?.tokensAfter ?? null;
		this.state.phase = "thinking";
		this.recomputePhase();
		// manuelles /compact startet keinen Agentenlauf, also kommt auch kein agent_settled: sofort wieder frei
		if (c.reason === "manual" && this.state.phase === "thinking") this.state.phase = "idle";
		this.event("compact", `Kontext gequetscht${c.tokensBefore ? ` von ${Math.round(c.tokensBefore / 1000)}k` : ""}`, now);
		this.touch(now);
	}

	compactFailed(msg: string, now: number) {
		this.state.compaction.active = false;
		this.event("compact_failed", `Compaction fehlgeschlagen: ${msg}`, now);
		this.error(`Compaction fehlgeschlagen: ${msg}`, now);
	}

	retryStart(attempt: number, max: number, error: string, now: number) {
		this.state.retry = { attempt, max, error: clip(error, 80) };
		this.state.phase = "retrying";
		this.event("retry", `Retry ${attempt}/${max}: ${error}`, now);
		this.touch(now);
	}

	retryEnd(success: boolean, now: number) {
		this.state.retry = null;
		if (!success) this.error("retries exhausted", now);
		this.touch(now);
	}

	settled(now: number) {
		this.state.tools = [];
		if (this.state.phase !== "error") this.state.phase = "idle";
		for (const r of this.state.subagentRuns) r.finished = true;
		this.touch(now);
	}

	snapshot(now: number): ScopeState {
		this.state.tokens.rateOut = this.rate(now);
		this.state.updated = now;
		return this.state;
	}
}
