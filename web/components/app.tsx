"use client";

import {
  BatteryWarning,
  Bluetooth,
  DownloadSimple,
  EarSlash,
  Ear,
  Equalizer,
  Headphones,
  HandTap,
  Heart,
  Lightning,
  LockSimple,
  Microphone,
  MicrophoneStage,
  Pause,
  Phone,
  PhoneIncoming,
  Play,
  Power,
  SlidersHorizontal,
  SpeakerHigh,
  UserFocus,
  UsersThree,
  Waveform,
  Wind,
  X,
} from "@phosphor-icons/react";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";

import type { Mode, View, WebAction } from "@/lib/core/core";
import { serialSupported } from "@/lib/link";
import { CONNECT_TIMEOUT_MS, Session, type Snapshot } from "@/lib/session";
import { Card, ChoiceRow, Divider, Hint, SectionTitle, Segmented, SwitchRow, cn } from "./ui";

const REPO = "https://github.com/xndbogdan/sennheiser-control";
// keep in step with `Product::ALL` in crates/protocol/src/product.rs (the page is static, the core loads on connect)
const MODELS = [
  "MOMENTUM 4",
  "ACCENTUM",
  "ACCENTUM Plus",
  "ACCENTUM True Wireless",
  "MOMENTUM True Wireless 3 and 4",
  "MOMENTUM Sport",
];
const session = new Session();
const IDLE: Snapshot = { status: "idle", simulated: false };

function useSession() {
  return useSyncExternalStore(session.subscribe, session.getSnapshot, () => IDLE);
}

const act = (a: WebAction) => session.act(a);

const noSubscription = () => () => {};

export default function App() {
  const s = useSession();
  // unknown (null) while prerendering on the server
  const supported = useSyncExternalStore(noSubscription, serialSupported, () => null);

  useEffect(() => {
    // reconnect to a headset this site was allowed to use before (no picker needed)
    if (serialSupported()) session.connect("known");
  }, []);

  const live = s.status === "connected" && s.view;
  return (
    // two columns from `lg` up once connected; the landing page stays one narrow column
    <div className={cn("mx-auto flex min-h-dvh w-full max-w-md flex-col px-4 pt-6 pb-10", live && "lg:max-w-5xl")}>
      <TopBar demo={s.simulated && s.status !== "idle"} />
      <main className="mt-6 flex flex-1 flex-col gap-4">
        {live ? (
          <Connected v={s.view!} demo={s.simulated} />
        ) : (
          <Landing snap={s} supported={supported} />
        )}
      </main>
      <Footer />
      {s.notice && (
        <div
          role="status"
          className="bg-card text-foreground fixed inset-x-4 bottom-6 mx-auto max-w-sm rounded-xl px-4 py-3 text-sm shadow-lg ring-1 ring-white/10"
        >
          {s.notice}
        </div>
      )}
    </div>
  );
}

function TopBar({ demo }: { demo: boolean }) {
  return (
    <header className="flex items-center gap-3">
      <div className="bg-primary/15 text-primary grid size-9 place-items-center rounded-xl">
        <Headphones size={20} weight="bold" />
      </div>
      <div className="flex-1 leading-tight">
        <div className="font-semibold">Sennheiser Control</div>
        <div className="text-muted-foreground text-xs">Unofficial · in your browser</div>
      </div>
      {demo && <span className="bg-warn/15 text-warn rounded-full px-2.5 py-1 text-xs font-medium">Demo</span>}
    </header>
  );
}

function Footer() {
  return (
    <footer className="mt-10 flex flex-col items-center gap-3 text-center">
      <div className="flex flex-wrap items-center justify-center gap-x-6 gap-y-2 text-sm">
        <a href={REPO} className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 transition-colors">
          <DownloadSimple size={16} /> Download desktop app
        </a>
        <a
          href="https://www.himthe.dev/"
          className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 transition-colors"
        >
          <Heart size={16} /> Made by Bobee
        </a>
      </div>
      <p className="text-faint text-xs">Not affiliated with or endorsed by Sennheiser or Sonova.</p>
    </footer>
  );
}

// ------------------------------------------------------------------------------------------------ landing

function Landing({ snap, supported }: { snap: Snapshot; supported: boolean | null }) {
  if (snap.status === "connecting") {
    return (
      <Card className="px-6 py-12">
        <Connecting simulated={snap.simulated} />
      </Card>
    );
  }
  return (
    <>
      <Card className="flex flex-col items-center px-6 py-10 text-center">
        <div className="bg-primary/10 text-primary mb-6 grid size-20 place-items-center rounded-3xl ring-1 ring-current/20">
          <Headphones size={42} />
        </div>
        <h1 className="text-2xl font-semibold tracking-tight">Your Sennheiser headphones, in the browser</h1>
        <p className="text-muted-foreground mt-2 max-w-xs">
          Noise control, EQ and headset settings, straight over Bluetooth. Nothing to install.
        </p>
        <p className="text-faint mt-4 max-w-xs text-xs">For {MODELS.join(" · ")}</p>
        <div className="mt-8 flex w-full flex-col gap-2">
          {supported !== false && (
            <button
              type="button"
              disabled={supported === null}
              onClick={() => session.connect("pick")}
              className="bg-primary text-primary-foreground inline-flex h-11 items-center justify-center gap-2 rounded-xl font-medium transition hover:brightness-110 disabled:opacity-50"
            >
              <Bluetooth size={18} weight="bold" />
              Connect headphones
            </button>
          )}
          <button
            type="button"
            onClick={() => session.connect("sim")}
            className="bg-muted hover:bg-accent inline-flex h-11 items-center justify-center gap-2 rounded-xl font-medium transition disabled:opacity-50"
          >
            <Play size={16} weight="fill" />
            Try the demo
          </button>
        </div>
        {snap.error && <p className="text-destructive mt-4 text-sm">{snap.error}</p>}
        {snap.status === "unsupported" && (
          <p className="text-warn mt-4 text-sm">
            {snap.view?.product ? `“${snap.view.product}” isn't` : "This headset isn't"} supported yet.
          </p>
        )}
      </Card>
      <Card>
        {supported === false ? (
          <p className="text-muted-foreground text-sm">
            This browser can&apos;t talk to Bluetooth headphones. Open this page in <b className="text-foreground">Chrome</b>{" "}
            or <b className="text-foreground">Edge</b> on a desktop computer, or try the demo.
          </p>
        ) : (
          <ol className="text-muted-foreground space-y-3 text-sm">
            {[
              "Pair the headphones with your computer and connect them.",
              "Click Connect headphones and pick them in the list.",
              "Works in Chrome and Edge on desktop. The browser remembers them for next time.",
            ].map((t, i) => (
              <li key={i} className="flex gap-3">
                <span className="bg-muted text-foreground grid size-6 shrink-0 place-items-center rounded-full text-xs font-medium">
                  {i + 1}
                </span>
                <span className="pt-0.5">{t}</span>
              </li>
            ))}
          </ol>
        )}
      </Card>
    </>
  );
}

/** Pulsing rings while the headset is reached; the bar runs out when the attempt is given up. */
function Connecting({ simulated }: { simulated: boolean }) {
  return (
    <div className="flex flex-col items-center text-center" role="status" aria-live="polite">
      <div className="relative mb-8 grid size-24 place-items-center">
        <span className="bg-primary/25 absolute inset-0 rounded-full motion-safe:animate-ping" />
        <span className="bg-primary/10 absolute -inset-4 rounded-full [animation-delay:500ms] motion-safe:animate-ping" />
        <div className="bg-card text-primary relative grid size-24 place-items-center rounded-full ring-1 ring-current/30">
          <Headphones size={44} />
        </div>
      </div>
      <h1 className="text-xl font-semibold tracking-tight">
        {simulated ? "Starting the demo…" : "Connecting to your headphones…"}
      </h1>
      <p className="text-muted-foreground mt-2 text-sm">This only takes a few seconds.</p>
      <div className="bg-muted mt-6 h-1 w-48 overflow-hidden rounded-full">
        <div className="bg-primary animate-connect h-full origin-left" style={{ animationDuration: `${CONNECT_TIMEOUT_MS}ms` }} />
      </div>
    </div>
  );
}

// ------------------------------------------------------------------------------------------------ connected

/** From `lg` up: the device across the top, what you hear on the left, how the headset behaves on the right. */
function Connected({ v, demo }: { v: View; demo: boolean }) {
  return (
    <div className="flex flex-col gap-4 lg:grid lg:grid-cols-2 lg:items-start">
      <div className="lg:col-span-2">
        <DeviceCard v={v} demo={demo} />
      </div>
      <div className="flex flex-col gap-4">
        {(v.noise.modes.length > 0 || v.noise.antiWind || v.noise.transparentHearing) && <NoiseCard v={v} />}
        <SoundCard v={v} />
      </div>
      <div className="flex flex-col gap-4">
        <HeadsetCard v={v} />
      </div>
    </div>
  );
}

function DeviceCard({ v, demo }: { v: View; demo: boolean }) {
  const chips = [
    v.codec && { icon: Waveform, text: v.codec },
    v.sampleRate !== undefined && { text: v.sampleRate === 0 ? "Idle" : `${+(v.sampleRate / 1000).toFixed(1)} kHz` },
    v.earbuds && { text: `L ${pct(v.earbuds.left)} · R ${pct(v.earbuds.right)}` },
    v.earbuds?.case !== undefined && { text: `Case ${pct(v.earbuds?.case)}` },
  ].filter(Boolean) as { icon?: typeof Waveform; text: string }[];
  const low = v.battery !== undefined && v.battery <= 15;
  return (
    <Card>
      <div className="flex items-start gap-4">
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-xl font-semibold tracking-tight">{v.product}</h1>
          <div className="text-muted-foreground mt-0.5 flex items-center gap-1.5 text-sm">
            <span className={cn("size-2 rounded-full", demo ? "bg-warn" : "bg-primary")} />
            {v.charging ? "Charging" : demo ? "Simulated" : "Connected"}
            {v.variant && ` · ${v.variant}`}
          </div>
          {chips.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-1.5">
              {chips.map(({ icon: I, text }) => (
                <span key={text} className="bg-muted inline-flex items-center gap-1 rounded-md px-2 py-1 font-mono text-xs">
                  {I && <I size={12} />}
                  {text}
                </span>
              ))}
            </div>
          )}
        </div>
        <Battery level={v.battery} charging={v.charging} low={low} />
      </div>
      {!v.verified && (
        <p className="bg-warn/10 text-warn mt-4 rounded-lg px-3 py-2 text-sm">
          This model hasn&apos;t been tested on real hardware yet.{" "}
          <a href={`${REPO}/issues`} className="underline">
            Tell me how it goes
          </a>
          .
        </p>
      )}
      <div className="mt-4 flex justify-end">
        <button
          type="button"
          onClick={() => session.disconnect()}
          className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 text-sm"
        >
          <X size={14} /> {demo ? "End demo" : "Disconnect"}
        </button>
      </div>
    </Card>
  );
}

const pct = (v: number | undefined) => (v === undefined ? "—" : `${v}%`);

/** The LCD readout from the resume site. */
function Battery({ level, charging, low }: { level: number | undefined; charging: boolean; low: boolean }) {
  return (
    <div
      className={cn(
        "bg-background/60 flex flex-col items-end rounded-xl px-3 py-2 ring-1 ring-white/5",
        low ? "text-destructive" : "text-primary",
      )}
      aria-label={level === undefined ? "Battery level unknown" : `Battery ${level}%`}
    >
      <div className="text-muted-foreground flex items-center gap-1 text-[10px] tracking-[0.2em] uppercase">
        {charging ? <Lightning size={11} weight="fill" className="text-primary" /> : low && <BatteryWarning size={11} />}
        Batt
      </div>
      <div className="flex items-baseline">
        <span className="lcd-glow font-lcd text-3xl">{level === undefined ? "--" : String(level).padStart(2, "!")}</span>
        <span className="font-lcd ml-1 text-sm">%</span>
      </div>
    </div>
  );
}

// ------------------------------------------------------------------------------------------------ noise

const MODE_HINT: Record<Mode, string> = {
  adaptive: "Noise cancelling adjusts itself to your surroundings.",
  custom: "",
  off: "Noise cancelling is off. Saves a little battery.",
  anc: "Blocks out the noise around you.",
  outdoor: "Noise cancelling that also reduces wind noise.",
  transparency: "Lets you hear your surroundings.",
};

const WIND = [
  { value: 0, label: "Off" },
  { value: 2, label: "Auto" },
  { value: 1, label: "Max" },
];

function NoiseCard({ v }: { v: View }) {
  const n = v.noise;
  return (
    <Card>
      <SectionTitle icon={Ear}>Noise control</SectionTitle>
      {n.modes.length > 0 && (
        <>
          <Segmented
            label="Noise control mode"
            options={n.modes.map((m) => ({ value: m.id, label: m.label }))}
            value={n.mode}
            disabled={n.mode === undefined}
            onChange={(mode) => act({ type: "noise", mode })}
          />
          {n.mode === "custom" ? <Balance value={n.balance} /> : n.mode && <Hint>{MODE_HINT[n.mode]}</Hint>}
        </>
      )}
      {n.transparentHearing && (
        <>
          {n.modes.length > 0 && <Divider />}
          <SwitchRow
            icon={Ear}
            title="Transparent hearing"
            subtitle="Hear your surroundings"
            value={n.transparentHearing.value}
            onChange={(on) => act({ type: "transparentHearing", on })}
          />
        </>
      )}
      {n.antiWind && (
        <>
          <Divider />
          <ChoiceRow
            icon={Wind}
            title="Wind noise reduction"
            options={WIND}
            value={n.mode === "off" ? undefined : n.antiWind.value}
            onChange={(value) => act({ type: "antiWind", value })}
          />
          {n.mode === "off" && <Hint>Works while noise cancelling is on.</Hint>}
        </>
      )}
    </Card>
  );
}

/** Local value while a slider is dragged; writes are throttled (the session also drops stale queued ones). */
function useDraft<T>(send: (v: T) => void, every = 100) {
  const [draft, setDraft] = useState<T | undefined>(undefined);
  const last = useRef(0);
  const clear = useRef<ReturnType<typeof setTimeout>>(undefined);
  const change = (v: T) => {
    clearTimeout(clear.current);
    setDraft(v);
    const now = performance.now();
    if (now - last.current >= every) {
      last.current = now;
      send(v);
    }
  };
  // send the final value; keep showing it until the headset has had time to confirm
  const commit = () => {
    if (draft === undefined) return;
    send(draft);
    clearTimeout(clear.current);
    clear.current = setTimeout(() => setDraft(undefined), 1200);
  };
  useEffect(() => () => clearTimeout(clear.current), []);
  return { draft, change, commit };
}

function balanceLabel(level: number) {
  if (level === 0) return "Max noise cancelling";
  if (level === 50) return "Neutral";
  if (level === 100) return "Max transparency";
  return level < 50 ? `Noise cancelling ${(50 - level) * 2}%` : `Transparency ${(level - 50) * 2}%`;
}

function Balance({ value }: { value: number | undefined }) {
  const { draft, change, commit } = useDraft<number>((v) => act({ type: "balance", value: v }));
  const level = draft ?? value ?? 50;
  return (
    <div className="mt-4">
      <div className="mb-2 text-sm font-medium">{balanceLabel(level)}</div>
      <input
        type="range"
        min={0}
        max={100}
        // Smart Control Plus snaps to the neutral centre
        step={1}
        value={level}
        disabled={value === undefined}
        aria-label="Noise cancelling to transparency balance"
        className="range centered w-full"
        onChange={(e) => {
          const v = +e.target.value;
          change(Math.abs(v - 50) <= 2 ? 50 : v);
        }}
        onPointerUp={commit}
        onKeyUp={commit}
      />
      <div className="text-faint mt-1 flex justify-between text-xs">
        <span>Noise cancelling</span>
        <span>Transparency</span>
      </div>
    </div>
  );
}

// ------------------------------------------------------------------------------------------------ sound

const BAND_LABELS = ["63", "250", "1k", "4k", "8k"];

function SoundCard({ v }: { v: View }) {
  const s = v.sound;
  const podcast = s.podcast === true;
  const editable = s.eq && s.known && !podcast;
  const { draft, change, commit } = useDraft<number[]>((bands) => act({ type: "eqBands", bands }), 120);
  // with the EQ off the headset plays its own tuning, so the curve is flat until one is chosen
  const bands = draft ?? (s.active ? s.bands : [0, 0, 0, 0, 0]);
  const chips = [
    ...(s.eqOff ? [{ name: "Neutral", apply: () => act({ type: "eqEnabled", on: false }) }] : []),
    ...s.presets.map((p) => ({ name: p.name, apply: () => act({ type: "eqBands", bands: p.bands }) })),
  ];
  return (
    <Card>
      <SectionTitle icon={Equalizer}>Sound</SectionTitle>
      <SwitchRow
        icon={MicrophoneStage}
        title="Podcast mode"
        subtitle="Clearer voices for podcasts and audiobooks"
        value={s.podcast}
        onChange={(on) => act({ type: "podcast", on })}
      />
      <Divider />
      {!s.eq ? (
        <p className="text-muted-foreground text-sm">The equalizer needs newer headset firmware.</p>
      ) : (
        <>
          <div className="flex flex-wrap gap-1.5" role="radiogroup" aria-label="EQ preset">
            {chips.map((c) => {
              const on = !draft && s.choice === c.name;
              return (
                <button
                  key={c.name}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  disabled={!editable}
                  onClick={c.apply}
                  className={cn(
                    "rounded-full px-3 py-1.5 text-sm transition-colors disabled:opacity-40",
                    on ? "bg-primary text-primary-foreground font-medium" : "bg-muted hover:bg-accent",
                  )}
                >
                  {c.name}
                </button>
              );
            })}
            {(draft || s.choice === "Custom") && (
              <span className="bg-primary/15 text-primary rounded-full px-3 py-1.5 text-sm font-medium">Custom</span>
            )}
          </div>
          <div className={cn("mt-5 grid grid-cols-5 gap-2", !editable && "opacity-40")}>
            {bands.map((db, i) => (
              <div key={i} className="flex flex-col items-center gap-2">
                <span className="text-muted-foreground font-mono text-xs tabular-nums">
                  {db > 0 ? "+" : ""}
                  {db.toFixed(1)}
                </span>
                <input
                  type="range"
                  min={s.minDb}
                  max={s.maxDb}
                  step={0.1}
                  value={db}
                  disabled={!editable}
                  aria-label={`${BAND_LABELS[i]} Hz`}
                  className="range vertical h-36"
                  onChange={(e) => {
                    const next = bands.slice();
                    next[i] = +e.target.value;
                    change(next);
                  }}
                  onPointerUp={commit}
                  onKeyUp={commit}
                />
                <span className="text-faint text-xs">{BAND_LABELS[i]}</span>
              </div>
            ))}
          </div>
          <Divider />
          <SwitchRow
            icon={SpeakerHigh}
            title="Bass boost"
            subtitle={s.active ? "More low end on top of the EQ" : "Works with the EQ on"}
            value={s.known ? s.bassBoost : undefined}
            disabled={!s.active || podcast}
            onChange={(on) => act({ type: "bassBoost", on })}
          />
        </>
      )}
    </Card>
  );
}

// ------------------------------------------------------------------------------------------------ settings

const SIDETONE = ["Off", "Low", "Medium", "High", "Max"];
const SIDETONE_BY_MAX: Record<number, string[]> = { 1: ["Off", "On"], 2: ["Off", "Low", "High"], 3: ["Off", "Low", "Medium", "High"] };
const AUTO_OFF = [
  { value: 0, label: "Never" },
  { value: 900, label: "15 min" },
  { value: 1800, label: "30 min" },
  { value: 3600, label: "60 min" },
];
const PROMPTS = [
  { value: 0, label: "Off" },
  { value: 1, label: "Tones" },
  { value: 2, label: "Voice + tones" },
];

function HeadsetCard({ v }: { v: View }) {
  const st = v.settings;
  // smart pause and auto-answer depend on on-head detection where the product has it
  const wearing = !st.needsOnHead || st.onHead?.value === true;
  const needs = (on: string) => (!st.needsOnHead ? on : wearing ? on : "Needs on-head detection");
  const sidetone = SIDETONE_BY_MAX[st.sidetoneMax] ?? SIDETONE;
  return (
    <>
      {(st.comfortCall || st.autoCall || st.sidetoneMax > 0) && (
        <Card>
          <SectionTitle icon={Phone}>Calls</SectionTitle>
          {st.comfortCall && (
            <SwitchRow
              icon={UsersThree}
              title="Comfort calls"
              subtitle="Hear your surroundings during calls"
              value={st.comfortCall.value}
              onChange={(on) => act({ type: "comfortCall", on })}
            />
          )}
          {st.autoCall && (
            <SwitchRow
              icon={PhoneIncoming}
              title="Auto-answer calls"
              subtitle={needs(st.needsOnHead ? "Answer by putting the headphones on" : "Answer incoming calls automatically")}
              value={st.autoCall.value}
              disabled={!wearing}
              onChange={(on) => act({ type: "autoCall", on })}
            />
          )}
          {st.sidetoneMax > 0 && (
            <ChoiceRow
              icon={Microphone}
              title="Sidetone"
              options={sidetone.map((label, value) => ({ value, label }))}
              value={st.sidetone === undefined ? undefined : Math.min(st.sidetone, st.sidetoneMax)}
              onChange={(value) => act({ type: "sidetone", value })}
            />
          )}
        </Card>
      )}
      <Card>
        <SectionTitle icon={SlidersHorizontal}>Headset</SectionTitle>
        {st.onHead && (
          <SwitchRow
            icon={UserFocus}
            title="On-head detection"
            subtitle="Plays, pauses and powers on as you wear them"
            value={st.onHead.value}
            onChange={(on) => act({ type: "onHead", on })}
          />
        )}
        {st.smartPause && (
          <SwitchRow
            icon={Pause}
            title="Smart pause"
            subtitle={needs(st.needsOnHead ? "Pause when you take them off" : "Pause when you take an earbud out")}
            value={st.smartPause.value}
            disabled={!wearing}
            onChange={(on) => act({ type: "smartPause", on })}
          />
        )}
        {st.touchLock && (
          <SwitchRow
            icon={LockSimple}
            title="Lock touch controls"
            subtitle="Ignore taps and swipes"
            value={st.touchLock.value}
            onChange={(on) => act({ type: "touchLock", on })}
          />
        )}
        {st.autoOff && (
          <ChoiceRow
            icon={Power}
            title="Auto power-off"
            options={AUTO_OFF}
            value={st.autoOff.value}
            onChange={(seconds) => act({ type: "autoOff", seconds })}
          />
        )}
        {st.prompts && (
          <ChoiceRow
            icon={HandTap}
            title="Tones and voice prompts"
            options={PROMPTS}
            value={st.prompts.value}
            onChange={(value) => act({ type: "prompts", value })}
          />
        )}
        {!st.onHead && !st.smartPause && !st.touchLock && !st.autoOff && !st.prompts && (
          <p className="text-muted-foreground flex items-center gap-2 text-sm">
            <EarSlash size={16} /> No settings for this model yet.
          </p>
        )}
      </Card>
    </>
  );
}
