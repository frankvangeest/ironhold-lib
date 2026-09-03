/* tslint:disable */
/* eslint-disable */

export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly start: () => void;
    readonly wasm_bindgen_c38e044b2bd95f76___closure__destroy___dyn_core_9b3796e30d99ddb7___ops__function__FnMut__core_9b3796e30d99ddb7___option__Option_web_sys_4ab3069b31b5f7a3___features__gen_Blob__Blob_____Output_______: (a: number, b: number) => void;
    readonly wasm_bindgen_c38e044b2bd95f76___closure__destroy___dyn_core_9b3796e30d99ddb7___ops__function__FnMut_____Output_______: (a: number, b: number) => void;
    readonly wasm_bindgen_c38e044b2bd95f76___closure__destroy___dyn_core_9b3796e30d99ddb7___ops__function__FnMut__wasm_bindgen_c38e044b2bd95f76___JsValue____Output_______: (a: number, b: number) => void;
    readonly wasm_bindgen_c38e044b2bd95f76___convert__closures_____invoke___js_sys_9033922e3b4734d9___Array__web_sys_4ab3069b31b5f7a3___features__gen_ResizeObserver__ResizeObserver_____: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_c38e044b2bd95f76___convert__closures_____invoke___js_sys_9033922e3b4734d9___Array_____: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_c38e044b2bd95f76___convert__closures_____invoke___core_9b3796e30d99ddb7___option__Option_web_sys_4ab3069b31b5f7a3___features__gen_Blob__Blob______: (a: number, b: number, c: number) => void;
    readonly wasm_bindgen_c38e044b2bd95f76___convert__closures_____invoke___wasm_bindgen_c38e044b2bd95f76___JsValue_____: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_c38e044b2bd95f76___convert__closures_____invoke______: (a: number, b: number) => void;
    readonly wasm_bindgen_c38e044b2bd95f76___convert__closures_____invoke_______1_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
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
