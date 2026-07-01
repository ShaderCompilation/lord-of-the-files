import { appLogDir } from "@tauri-apps/api/path";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import * as s from "../../store";

async function openLogs() {
  try {
    const dir = await appLogDir();
    await revealItemInDir(`${dir}/lord-of-the-files.log`);
  } catch (e) {
    s.setNotice(`Could not open logs: ${String(e)}`);
  }
}

export function GeneralSettings() {
  return (
    <section class="settings-general">
      <h3>General</h3>
      <label class="check">
        <input
          type="checkbox"
          checked={s.settings().debugLogging}
          onChange={(e) => void s.setDebugLogging(e.currentTarget.checked)}
        />
        Enable debug logs
      </label>
      <p class="muted small hint">
        Enable to capture detailed logs, then share the file for bug reports.
      </p>
      <button type="button" class="ghost small" onClick={() => void openLogs()}>
        Open logs
      </button>
    </section>
  );
}
