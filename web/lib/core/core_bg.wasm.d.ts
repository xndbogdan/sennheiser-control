/* tslint:disable */
/* eslint-disable */
export const memory: WebAssembly.Memory;
export const __wbg_core_free: (a: number, b: number) => void;
export const __wbg_sim_free: (a: number, b: number) => void;
export const core_abandon: (a: number, b: number) => void;
export const core_begin: (a: number, b: number) => void;
export const core_create: () => number;
export const core_dropRequest: (a: number, b: number) => void;
export const core_feed: (a: number, b: number, c: number) => [number, number, number];
export const core_identified: (a: number) => number;
export const core_identityRequests: (a: number) => [number, number, number];
export const core_plan: (a: number, b: any) => [number, number, number];
export const core_pollRequests: (a: number) => [number, number, number];
export const core_refreshRequests: (a: number) => [number, number, number];
export const core_view: (a: number) => [number, number, number];
export const sim_new: () => number;
export const sim_receive: (a: number, b: number, c: number) => [number, number];
export const sim_tick: (a: number) => [number, number];
export const __wbindgen_malloc: (a: number, b: number) => number;
export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
export const __wbindgen_free: (a: number, b: number, c: number) => void;
export const __wbindgen_exn_store: (a: number) => void;
export const __externref_table_alloc: () => number;
export const __wbindgen_externrefs: WebAssembly.Table;
export const __externref_table_dealloc: (a: number) => void;
export const __wbindgen_start: () => void;
