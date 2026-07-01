import { For, Show } from "solid-js";

import * as s from "../../store";
import { Button } from "../common";
import { GeneralSettings } from "./GeneralSettings";
import { ProfileCard } from "./ProfileCard";

export function ProfileList(props: { onAdd: () => void; onEdit: (id: string) => void }) {
  return (
    <>
      <GeneralSettings />

      <p class="muted small settings-intro">
        Bring your own key — configure a provider to use AI Rename.
      </p>
      <Button variant="primary" class="settings-add" onClick={props.onAdd}>
        + Add provider
      </Button>

      <Show
        when={s.settings().profiles.length > 0}
        fallback={<p class="muted hint">No providers yet — add one to start using AI Rename.</p>}
      >
        <ul class="provider-list">
          <For each={s.settings().profiles}>
            {(profile) => <ProfileCard profile={profile} onEdit={() => props.onEdit(profile.id)} />}
          </For>
        </ul>
      </Show>
    </>
  );
}
