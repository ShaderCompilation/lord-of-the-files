// Thin re-export of the Tauri log plugin (mirrors the `ipc.ts` convention: single import
// site, easy to no-op in tests). REDACTION: never pass secrets (API keys, auth headers) to
// these — they are written to the on-disk log file.

import { trace, debug, info, warn, error } from "@tauri-apps/plugin-log";

export const log = { trace, debug, info, warn, error };
