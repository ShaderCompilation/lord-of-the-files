import { appLogDir } from "@tauri-apps/api/path";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import * as s from "../../store";
import { Button, Checkbox } from "../common";

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
      <Checkbox
        checked={s.settings().debugLogging}
        onChange={(v) => void s.setDebugLogging(v)}
      >
        Enable debug logs
      </Checkbox>
      <p class="muted small hint">
        Enable to capture detailed logs, then share the file for bug reports.
      </p>
      <Button variant="ghost" small onClick={() => void openLogs()}>
        Open logs
      </Button>
    </section>
  );
}
