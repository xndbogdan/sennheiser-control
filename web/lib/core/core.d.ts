/* tslint:disable */
/* eslint-disable */
/**
 * A request ready to write. Hand `token` back to [`Core::begin`] right before writing `bytes`.
 */
export interface OutRequest {
    token: number;
    bytes: Uint8Array;
    /**
     * Queued requests with the same key replace each other (slider drags); only the latest is worth sending.
     */
    coalesce: string | undefined;
    /**
     * A write the user asked for (a rejection is worth telling them about).
     */
    write: boolean;
}

/**
 * A setting the product has; `value` is `None` until it has been read. (Absent when the product lacks it.)
 */
export interface Setting<T> {
    value: T | undefined;
}

/**
 * Everything the UI shows. `None` means not read yet.
 */
export interface View {
    /**
     * The model id belongs to a product this app supports.
     */
    supported: boolean;
    /**
     * Checked on real hardware (only the MOMENTUM 4 so far).
     */
    verified: boolean;
    product: string;
    modelId: string | undefined;
    variant: string | undefined;
    firmware: string | undefined;
    battery: number | undefined;
    earbuds: Earbuds | undefined;
    charging: boolean;
    codec: string | undefined;
    sampleRate: number | undefined;
    noise: NoiseView;
    sound: SoundView;
    settings: SettingsView;
}

/**
 * UI actions the web app offers (a subset of [`Action`]).
 */
export type WebAction = { type: "noise"; mode: Mode } | { type: "balance"; value: number } | { type: "antiWind"; value: number } | { type: "transparentHearing"; on: boolean } | { type: "eqBands"; bands: number[] } | { type: "eqEnabled"; on: boolean } | { type: "bassBoost"; on: boolean } | { type: "podcast"; on: boolean } | { type: "onHead"; on: boolean } | { type: "smartPause"; on: boolean } | { type: "autoCall"; on: boolean } | { type: "comfortCall"; on: boolean } | { type: "touchLock"; on: boolean } | { type: "sidetone"; value: number } | { type: "autoOff"; seconds: number } | { type: "prompts"; value: number };

export interface Batch {
    requests: OutRequest[];
}

export interface Earbuds {
    left: number | undefined;
    right: number | undefined;
    case: number | undefined;
}

export interface FeedResult {
    /**
     * The in-flight request this data answered (accepted or rejected).
     */
    done: number | undefined;
    rejected: Rejection | undefined;
    /**
     * Reads to queue next (the paired-device entries once their count is known).
     */
    followUps: OutRequest[];
    /**
     * The state changed: re-read [`Core::view`].
     */
    changed: boolean;
}

export interface ModeOption {
    id: Mode;
    label: string;
}

export interface NoiseView {
    modes: ModeOption[];
    mode: Mode | undefined;
    /**
     * Custom-mode balance (`ANC_Transparency`).
     */
    balance: number | undefined;
    /**
     * `Some` when the product has a separate wind setting: 0 off, 1 max, 2 auto.
     */
    antiWind: Setting<number> | undefined;
    /**
     * `Some` when the product has a separate transparent hearing switch.
     */
    transparentHearing: Setting<boolean> | undefined;
}

export interface Preset {
    name: string;
    bands: number[];
}

export interface Rejection {
    request: string;
    code: number;
    userChange: boolean;
}

export interface SettingsView {
    /**
     * Absent when the product lacks the setting.
     */
    onHead: Setting<boolean> | undefined;
    smartPause: Setting<boolean> | undefined;
    /**
     * Smart pause and auto-answer only work while on-head detection is on.
     */
    needsOnHead: boolean;
    autoCall: Setting<boolean> | undefined;
    comfortCall: Setting<boolean> | undefined;
    touchLock: Setting<boolean> | undefined;
    sidetoneMax: number;
    sidetone: number | undefined;
    autoOff: Setting<number> | undefined;
    prompts: Setting<number> | undefined;
}

export interface SoundView {
    /**
     * 5-band EQ available on this firmware.
     */
    eq: boolean;
    /**
     * "Neutral" (EQ off) is a mode this firmware knows.
     */
    eqOff: boolean;
    /**
     * All five bands have been read.
     */
    known: boolean;
    bands: number[];
    minDb: number;
    maxDb: number;
    bassBoost: boolean;
    /**
     * The user EQ is what's playing (otherwise Neutral or podcast mode).
     */
    active: boolean;
    podcast: boolean | undefined;
    presets: Preset[];
    /**
     * "Neutral", a preset name, or "Custom"; `None` in podcast mode.
     */
    choice: string | undefined;
}

export type Mode = "adaptive" | "custom" | "off" | "anc" | "outdoor" | "transparency";


/**
 * The shared core for one connection.
 */
export class Core {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * No response in time: stop waiting for it.
     */
    abandon(token: number): void;
    /**
     * The request is about to be written; its response is matched from here on.
     */
    begin(token: number): void;
    constructor();
    /**
     * The queue dropped this request (a newer write replaced it).
     */
    dropRequest(token: number): void;
    /**
     * Bytes received from the headset.
     */
    feed(bytes: Uint8Array | number[]): FeedResult;
    /**
     * The model id has been read and belongs to a supported product.
     */
    identified(): boolean;
    /**
     * Firmware version and model id: what identifies the headset. Queue these first.
     */
    identityRequests(): Batch;
    /**
     * The writes behind a UI action, given the current state.
     */
    plan(action: WebAction): Batch;
    /**
     * Battery level and charging state.
     */
    pollRequests(): Batch;
    /**
     * Notification registrations and every read behind the UI. Queue once the identity is known.
     */
    refreshRequests(): Batch;
    view(): View;
}

/**
 * The simulated MOMENTUM 4 from the desktop app's `--sim`, for the web demo.
 */
export class Sim {
    free(): void;
    [Symbol.dispose](): void;
    constructor();
    /**
     * Bytes written to the headset; returns what it sends back.
     */
    receive(bytes: Uint8Array | number[]): Uint8Array;
    /**
     * Drain the battery by 1 % (call every few seconds for a lively demo).
     */
    tick(): Uint8Array;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_core_free: (a: number, b: number) => void;
    readonly __wbg_sim_free: (a: number, b: number) => void;
    readonly core_abandon: (a: number, b: number) => void;
    readonly core_begin: (a: number, b: number) => void;
    readonly core_create: () => number;
    readonly core_dropRequest: (a: number, b: number) => void;
    readonly core_feed: (a: number, b: number, c: number) => [number, number, number];
    readonly core_identified: (a: number) => number;
    readonly core_identityRequests: (a: number) => [number, number, number];
    readonly core_plan: (a: number, b: any) => [number, number, number];
    readonly core_pollRequests: (a: number) => [number, number, number];
    readonly core_refreshRequests: (a: number) => [number, number, number];
    readonly core_view: (a: number) => [number, number, number];
    readonly sim_new: () => number;
    readonly sim_receive: (a: number, b: number, c: number) => [number, number];
    readonly sim_tick: (a: number) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
