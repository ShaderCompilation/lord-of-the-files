import { Show, createEffect, createSignal } from "solid-js";

import * as s from "../../store";
import { ProfileForm } from "./ProfileForm";
import { ProfileList } from "./ProfileList";
import { createProfileEditor } from "./profileEditor";

export function SettingsPanel(props: { open: boolean; onClose: () => void }) {
  const [view, setView] = createSignal<"list" | "form">("list");
  const [editingId, setEditingId] = createSignal<string | null>(null);
  const editor = createProfileEditor();

  // Always land on the list when the panel is (re)opened.
  createEffect(() => {
    if (props.open) setView("list");
  });

  function openAdd() {
    setEditingId(null);
    editor.begin(null);
    setView("form");
  }

  function openEdit(id: string) {
    const profile = s.settings().profiles.find((p) => p.id === id);
    if (!profile) return;
    setEditingId(id);
    editor.begin(profile);
    setView("form");
  }

  function back() {
    setView("list");
  }

  const headTitle = () =>
    view() === "list" ? "Settings" : editingId() ? "Edit provider" : "Add a provider";

  return (
    <Show when={props.open}>
      <button type="button" class="overlay" aria-label="Close settings" onClick={props.onClose} />
      <aside class="settings-panel">
        <div class="settings-head">
          <div class="settings-head-left">
            <Show when={view() === "form"}>
              <button type="button" class="icon" onClick={back} title="Back">
                ←
              </button>
            </Show>
            <h2>{headTitle()}</h2>
          </div>
          <button type="button" class="icon" onClick={props.onClose} title="Close">
            ✕
          </button>
        </div>

        <div class="settings-body">
          <Show when={view() === "list"} fallback={<ProfileForm editor={editor} onDone={back} />}>
            <ProfileList onAdd={openAdd} onEdit={openEdit} />
          </Show>
        </div>
      </aside>
    </Show>
  );
}
