import { For, Show, createEffect, createSignal } from "solid-js";

import { Button, Checkbox, Overlay } from "./common";

type SaveMode = "current" | "new" | "existing";

export function SavePresetDialog(props: {
  open: boolean;
  presetNames: string[];
  currentName: string;
  onCancel: () => void;
  onConfirm: (name: string) => void;
}) {
  const [mode, setMode] = createSignal<SaveMode>("new");
  const [newName, setNewName] = createSignal("");
  const [existingName, setExistingName] = createSignal("");

  const otherPresets = () =>
    props.presetNames.filter((n) => n !== props.currentName);

  // Reset to sensible defaults each time the dialog is (re)opened.
  createEffect(() => {
    if (!props.open) return;
    setMode(props.currentName ? "current" : "new");
    setNewName("");
    setExistingName(otherPresets()[0] ?? "");
  });

  const targetName = () => {
    if (mode() === "current") return props.currentName;
    if (mode() === "existing") return existingName();
    return newName().trim();
  };

  const canConfirm = () => targetName().length > 0;

  const confirm = () => {
    if (canConfirm()) props.onConfirm(targetName());
  };

  return (
    <Show when={props.open}>
      <Overlay ariaLabel="Close save preset dialog" onClick={props.onCancel} />
      <div class="save-preset-dialog" role="dialog" aria-label="Save preset">
        <h3>Save preset</h3>

        <div class="save-preset-options">
          <Show when={props.currentName}>
            <Checkbox
              type="radio"
              name="save-preset-mode"
              checked={mode() === "current"}
              onChange={() => setMode("current")}
            >
              Save to "{props.currentName}"
            </Checkbox>
          </Show>

          <Checkbox
            type="radio"
            name="save-preset-mode"
            checked={mode() === "new"}
            onChange={() => setMode("new")}
          >
            Save as new preset
          </Checkbox>
          <Show when={mode() === "new"}>
            <input
              class="save-preset-input"
              type="text"
              placeholder="Preset name"
              value={newName()}
              onInput={(e) => setNewName(e.currentTarget.value)}
              autofocus
            />
          </Show>

          <Show when={otherPresets().length > 0}>
            <Checkbox
              type="radio"
              name="save-preset-mode"
              checked={mode() === "existing"}
              onChange={() => setMode("existing")}
            >
              {props.currentName ? "Update another preset" : "Update existing preset"}
            </Checkbox>
            <Show when={mode() === "existing"}>
              <select
                class="save-preset-input"
                value={existingName()}
                onChange={(e) => setExistingName(e.currentTarget.value)}
              >
                <For each={otherPresets()}>
                  {(name) => <option value={name}>{name}</option>}
                </For>
              </select>
            </Show>
          </Show>
        </div>

        <div class="save-preset-actions">
          <Button variant="primary" disabled={!canConfirm()} onClick={confirm}>
            Save
          </Button>
          <Button variant="ghost" onClick={props.onCancel}>
            Cancel
          </Button>
        </div>
      </div>
    </Show>
  );
}
