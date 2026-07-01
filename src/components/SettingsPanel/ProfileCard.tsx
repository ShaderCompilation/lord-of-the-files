import { Show, createSignal } from "solid-js";

import type { ProviderProfile } from "../../lib/types";
import * as s from "../../store";

export function ProfileCard(props: { profile: ProviderProfile; onEdit: () => void }) {
  const [confirming, setConfirming] = createSignal(false);
  const active = () => props.profile.id === s.settings().activeProfileId;

  return (
    <li class="provider-card" classList={{ active: active() }}>
      <label class="check provider-active" title="Use for AI Rename">
        <input
          type="radio"
          name="active-profile"
          checked={active()}
          onChange={() => s.setActiveProfile(props.profile.id)}
        />
      </label>
      <button type="button" class="provider-main" onClick={props.onEdit} title="Edit">
        <span class="provider-name">{props.profile.label || "(unnamed)"}</span>
        <span class="muted small mono">{props.profile.model || "no model set"}</span>
      </button>
      <div class="provider-side">
        <Show when={props.profile.hasKey} fallback={<span class="muted small">No key</span>}>
          <span class="badge badge-changed">Key ✓</span>
        </Show>
        <Show
          when={confirming()}
          fallback={
            <button
              type="button"
              class="icon danger"
              title="Delete profile"
              onClick={() => setConfirming(true)}
            >
              🗑
            </button>
          }
        >
          <button
            type="button"
            class="small confirm-delete"
            onClick={() => s.deleteProfile(props.profile.id)}
          >
            Delete?
          </button>
          <button type="button" class="icon" title="Cancel" onClick={() => setConfirming(false)}>
            ✕
          </button>
        </Show>
      </div>
    </li>
  );
}
