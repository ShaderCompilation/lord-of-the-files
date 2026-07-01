import { Show } from "solid-js";

import { Button, Overlay } from "../common";
import { MockAiSection } from "./MockAiSection";

/** Dev-build-only panel for internal test tooling. Add future dev tools as more sections here. */
export function DevMenu(props: { open: boolean; onClose: () => void }) {
  return (
    <Show when={props.open}>
      <Overlay ariaLabel="Close dev menu" onClick={props.onClose} />
      <aside class="dev-panel">
        <div class="dev-panel-head">
          <h2>Dev menu</h2>
          <Button variant="icon" onClick={props.onClose} title="Close">
            ✕
          </Button>
        </div>
        <div class="dev-panel-body">
          <MockAiSection />
        </div>
      </aside>
    </Show>
  );
}
