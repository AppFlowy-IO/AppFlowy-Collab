import * as wasm from "./collab_plugins_wasm_bg.wasm";
import { __wbg_set_wasm } from "./collab_plugins_wasm_bg.js";
__wbg_set_wasm(wasm);
export * from "./collab_plugins_wasm_bg.js";
