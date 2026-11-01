// One connection to a headset. The shared Rust core (lib/core, built from crates/web) does the protocol work;
// this class moves bytes, keeps requests in order (one in flight, as the desktop client does) and times them out.

import init, { Core, Sim, type OutRequest, type View, type WebAction } from "./core/core";
import { knownPort, pickPort, serialLink, simLink, type Link, type LinkHandlers } from "./link";

export type Status = "idle" | "connecting" | "connected" | "unsupported";

export interface Snapshot {
  status: Status;
  /** Talking to the simulated headset (development only, `?sim`). */
  simulated: boolean;
  view?: View;
  /** Shown until the next attempt. */
  error?: string;
  /** Brief message (a rejected change); clears itself. */
  notice?: string;
}

/** How long a connection may take from picking the headset to having read it. Longer means it won't happen. */
export const CONNECT_TIMEOUT_MS = 10_000;
const COULD_NOT_CONNECT =
  "Could not connect to the headphones. Make sure they're turned on and connected to this computer, then try again.";
const TIMEOUT_MS = 1500;
const MAX_TIMEOUTS = 3;
// Smart Control Plus re-reads the battery every 60 s
const POLL_MS = 60_000;

let coreReady: Promise<unknown> | undefined;
const loadCore = () => (coreReady ??= init());

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class Session {
  private snap: Snapshot = { status: "idle", simulated: false };
  private listeners = new Set<() => void>();
  private core?: Core;
  private link?: Link;
  private queue: OutRequest[] = [];
  private inFlight?: { token: number; timer: ReturnType<typeof setTimeout> };
  private timeouts = 0;
  private identity = new Set<number>();
  private refreshed = false;
  private poll?: ReturnType<typeof setInterval>;
  private noticeTimer?: ReturnType<typeof setTimeout>;
  private connectTimer?: ReturnType<typeof setTimeout>;
  /** Bumped on every attempt and when one is given up, so late results of an old attempt are ignored. */
  private attempt = 0;
  private closing = false;

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  getSnapshot = () => this.snap;

  private set(patch: Partial<Snapshot>) {
    this.snap = { ...this.snap, ...patch };
    this.listeners.forEach((l) => l());
  }

  /** `pick` opens Chrome's device picker, `known` reuses a headset this site may already use, `sim` simulates one. */
  async connect(how: "pick" | "known" | "sim") {
    if (this.link || this.snap.status === "connecting") return;
    const attempt = ++this.attempt;
    let port: SerialPort | undefined;
    if (how !== "sim") {
      try {
        port = how === "pick" ? await pickPort() : await knownPort();
      } catch (e) {
        // closing the picker without choosing is not an error
        if (!(e instanceof DOMException && e.name === "NotFoundError")) this.set({ error: friendly(message(e)) });
        return;
      }
      if (!port || attempt !== this.attempt) return;
    }
    // the clock starts once there is a headset to talk to (not while the picker is open)
    this.set({ status: "connecting", simulated: how === "sim", view: undefined, error: undefined });
    this.connectTimer = setTimeout(() => this.giveUp(attempt), CONNECT_TIMEOUT_MS);
    try {
      await loadCore();
      const on: LinkHandlers = { data: (b) => this.onData(b), closed: (r) => this.onClosed(r) };
      const link = port ? await serialLink(port, on) : simLink(new Sim(), on);
      if (attempt !== this.attempt) {
        // gave up while the port was opening
        link.close();
        return;
      }
      this.link = link;
      this.closing = false;
      this.core = new Core();
      const batch = this.core.identityRequests().requests;
      batch.forEach((r) => this.identity.add(r.token));
      this.enqueue(batch);
    } catch {
      if (attempt === this.attempt) this.fail(COULD_NOT_CONNECT);
    }
  }

  async disconnect() {
    this.attempt++;
    this.closing = true;
    const link = this.link;
    this.teardown();
    await link?.close();
    this.set({ status: "idle", view: undefined });
  }

  act(action: WebAction) {
    if (this.snap.status !== "connected" || !this.core) return;
    // user changes go ahead of background reads, like the desktop app's high-priority queue
    this.enqueue(this.core.plan(action).requests, true);
  }

  private giveUp(attempt: number) {
    if (attempt === this.attempt && this.snap.status === "connecting") this.fail(COULD_NOT_CONNECT);
  }

  private onData(bytes: Uint8Array) {
    const core = this.core;
    if (!core) return;
    const res = core.feed(bytes);
    if (res.done !== undefined && res.done === this.inFlight?.token) {
      clearTimeout(this.inFlight.timer);
      this.inFlight = undefined;
      this.timeouts = 0;
      this.identity.delete(res.done);
    }
    if (res.rejected) {
      console.warn(`headset rejected ${res.rejected.request} (error ${res.rejected.code})`);
      if (res.rejected.userChange) this.flash("The headset turned that change down.");
    }
    if (res.followUps.length) this.enqueue(res.followUps);
    if (res.changed && this.snap.status === "connected") this.set({ view: core.view() });
    this.afterIdentity();
    this.pump();
  }

  private onClosed(reason?: string) {
    if (this.closing) return;
    if (this.snap.status === "connecting") return this.fail(COULD_NOT_CONNECT);
    this.teardown();
    this.set({ status: "idle", view: undefined, error: reason ? friendly(reason) : "The headphones disconnected." });
  }

  /** Once firmware and model id are in: read everything for a supported product, or stop. */
  private afterIdentity() {
    const core = this.core;
    if (!core || this.refreshed || this.identity.size) return;
    this.refreshed = true;
    clearTimeout(this.connectTimer);
    if (!core.identified()) {
      const view = core.view();
      this.disconnect().then(() => this.set({ status: "unsupported", view }));
      return;
    }
    this.enqueue(core.refreshRequests().requests);
    this.poll = setInterval(() => this.core && this.enqueue(this.core.pollRequests().requests), POLL_MS);
    this.set({ status: "connected", view: core.view() });
  }

  private enqueue(reqs: OutRequest[], front = false) {
    for (const r of reqs) {
      if (!r.coalesce) continue;
      // only the latest value of a slider drag is worth sending
      const i = this.queue.findIndex((q) => q.coalesce === r.coalesce);
      if (i >= 0) {
        this.core?.dropRequest(this.queue[i].token);
        this.queue.splice(i, 1);
      }
    }
    if (front) this.queue.unshift(...reqs);
    else this.queue.push(...reqs);
    this.pump();
  }

  private pump() {
    if (this.inFlight || !this.link || !this.core) return;
    const r = this.queue.shift();
    if (!r) return;
    this.core.begin(r.token);
    this.inFlight = { token: r.token, timer: setTimeout(() => this.onTimeout(r.token), TIMEOUT_MS) };
    this.link.write(r.bytes).catch((e) => this.fail(message(e)));
  }

  private onTimeout(token: number) {
    if (this.inFlight?.token !== token) return;
    this.core?.abandon(token);
    this.inFlight = undefined;
    // no firmware version or model id: there's nothing to talk to
    if (this.identity.has(token)) return this.fail(COULD_NOT_CONNECT);
    if (++this.timeouts >= MAX_TIMEOUTS) return this.fail("The headphones stopped responding.");
    this.afterIdentity();
    this.pump();
  }

  /** Drop the connection and say why; anything going wrong while connecting is "could not connect". */
  private fail(error: string) {
    const link = this.link;
    const connecting = this.snap.status === "connecting";
    this.attempt++;
    this.closing = true;
    this.teardown();
    link?.close();
    this.set({ status: "idle", view: undefined, error: connecting ? COULD_NOT_CONNECT : error });
  }

  private flash(notice: string) {
    clearTimeout(this.noticeTimer);
    this.set({ notice });
    this.noticeTimer = setTimeout(() => this.set({ notice: undefined }), 5000);
  }

  private teardown() {
    clearTimeout(this.inFlight?.timer);
    clearTimeout(this.connectTimer);
    clearInterval(this.poll);
    this.inFlight = undefined;
    this.queue = [];
    this.identity.clear();
    this.refreshed = false;
    this.timeouts = 0;
    this.core?.free();
    this.core = undefined;
    this.link = undefined;
  }
}

/** Web Serial errors are terse; say what to do instead. */
function friendly(msg: string) {
  if (/failed to open|network|connect/i.test(msg)) return COULD_NOT_CONNECT;
  return msg;
}
