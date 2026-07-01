import { Show, createSignal } from "solid-js";

import type { ProviderProfile } from "../../lib/types";
import * as s from "../../store";
import { Badge, Button, Checkbox } from "../common";

export function ProfileCard(props: { profile: ProviderProfile; onEdit: () => void }) {
  const [confirming, setConfirming] = createSignal(false);
  const active = () => props.profile.id === s.settings().activeProfileId;

  return (
    <li class="provider-card" classList={{ active: active() }}>
      <Checkbox
        type="radio"
        name="active-profile"
        class="provider-active"
        title="Use for AI Rename"
        checked={active()}
        onChange={() => s.setActiveProfile(props.profile.id)}
      />
      <Button class="provider-main" onClick={props.onEdit} title="Edit">
        <span class="provider-name">{props.profile.label || "(unnamed)"}</span>
        <span class="muted small mono">{props.profile.model || "no model set"}</span>
      </Button>
      <div class="provider-side">
        <Show when={props.profile.hasKey} fallback={<span class="muted small">No key</span>}>
          <Badge variant="changed">Key ✓</Badge>
        </Show>
        <Show
          when={confirming()}
          fallback={
            <Button variant="icon" danger title="Delete profile" onClick={() => setConfirming(true)}>
              🗑
            </Button>
          }
        >
          <Button small class="confirm-delete" onClick={() => s.deleteProfile(props.profile.id)}>
            Delete?
          </Button>
          <Button variant="icon" title="Cancel" onClick={() => setConfirming(false)}>
            ✕
          </Button>
        </Show>
      </div>
    </li>
  );
}
