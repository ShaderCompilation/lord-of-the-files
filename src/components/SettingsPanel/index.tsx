import { Show, createEffect, createSignal } from "solid-js";

import * as s from "../../store";
import { Button, Overlay } from "../common";
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
      <Overlay ariaLabel="Close settings" onClick={props.onClose} />
      <aside class="settings-panel">
        <div class="settings-head">
          <div class="settings-head-left">
            <Show when={view() === "form"}>
              <Button variant="icon" onClick={back} title="Back">
                ←
              </Button>
            </Show>
            <h2>{headTitle()}</h2>
          </div>
          <Button variant="icon" onClick={props.onClose} title="Close">
            ✕
          </Button>
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
