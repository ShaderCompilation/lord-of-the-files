import { For, Show, createEffect, createSignal } from "solid-js";

import { PROVIDER_PRESETS } from "../lib/providers";
import type { ProviderProfile } from "../lib/types";
import * as s from "../store";

function blankProfile(): ProviderProfile {
  const preset = PROVIDER_PRESETS[0];
  return {
    id: crypto.randomUUID(),
    label: preset.label,
    baseUrl: preset.baseUrl,
    model: preset.defaultModel,
    chunkSize: 40,
    concurrency: 3,
    maxLen: 80,
    timeoutSecs: 60,
    hasKey: false,
  };
}

export function SettingsPanel(props: { open: boolean; onClose: () => void }) {
  const [selectedId, setSelectedId] = createSignal<string | null>(null);
  const [draft, setDraft] = createSignal<ProviderProfile | null>(null);
  const [keyInput, setKeyInput] = createSignal("");
  const [advancedOpen, setAdvancedOpen] = createSignal(false);
  const [testStatus, setTestStatus] = createSignal<{ ok: boolean; message: string } | null>(null);
  const [testing, setTesting] = createSignal(false);

  function selectProfile(id: string | null) {
    setSelectedId(id);
    const profile = id ? s.settings().profiles.find((p) => p.id === id) : undefined;
    setDraft(profile ? { ...profile } : null);
    setKeyInput("");
    setTestStatus(null);
  }

  // Keep a valid selection whenever the panel opens or the profile list changes underneath it.
  createEffect(() => {
    if (!props.open) return;
    const profiles = s.settings().profiles;
    if (selectedId() !== null && profiles.some((p) => p.id === selectedId())) return;
    selectProfile(profiles[0]?.id ?? null);
  });

  function addProfile() {
    const profile = blankProfile();
    setSelectedId(profile.id);
    setDraft(profile);
    setKeyInput("");
    setTestStatus(null);
  }

  function applyPreset(label: string) {
    const preset = PROVIDER_PRESETS.find((p) => p.label === label);
    const current = draft();
    if (!preset || !current) return;
    setDraft({ ...current, label: preset.label, baseUrl: preset.baseUrl, model: preset.defaultModel });
  }

  async function save() {
    const d = draft();
    if (!d) return;
    await s.upsertProfile(d);
    if (keyInput()) {
      await s.saveApiKey(d.id, keyInput());
    }
    selectProfile(d.id);
  }

  async function remove(id: string) {
    await s.deleteProfile(id);
    setSelectedId(null);
  }

  async function testConn(id: string) {
    setTesting(true);
    setTestStatus(null);
    try {
      await s.testConnection(id);
      setTestStatus({ ok: true, message: "ok" });
    } catch (e) {
      setTestStatus({ ok: false, message: String(e) });
    } finally {
      setTesting(false);
    }
  }

  return (
    <Show when={props.open}>
      <button type="button" class="overlay" aria-label="Close settings" onClick={props.onClose} />
      <aside class="settings-panel">
        <div class="settings-head">
          <h2>Settings</h2>
          <button type="button" class="icon" onClick={props.onClose} title="Close">
            ✕
          </button>
        </div>

        <div class="settings-body">
          <div class="settings-profiles">
            <select
              value={selectedId() ?? ""}
              onChange={(e) => selectProfile(e.currentTarget.value || null)}
            >
              <option value="" disabled>
                Select a profile…
              </option>
              <For each={s.settings().profiles}>
                {(p) => (
                  <option value={p.id}>
                    {p.label}
                    {p.id === s.settings().activeProfileId ? " (active)" : ""}
                  </option>
                )}
              </For>
            </select>
            <button type="button" class="ghost small" onClick={addProfile}>
              Add profile
            </button>
          </div>

          <Show
            when={draft()}
            fallback={
              <p class="muted hint">No provider configured yet. Add one to use AI Rename.</p>
            }
          >
            {(d) => (
              <>
                <label class="field">
                  Preset
                  <select value={d().label} onChange={(e) => applyPreset(e.currentTarget.value)}>
                    <For each={PROVIDER_PRESETS}>
                      {(preset) => <option value={preset.label}>{preset.label}</option>}
                    </For>
                  </select>
                </label>
                <label class="field">
                  Label
                  <input
                    value={d().label}
                    onInput={(e) => setDraft({ ...d(), label: e.currentTarget.value })}
                  />
                </label>
                <label class="field">
                  Base URL
                  <input
                    class="mono"
                    value={d().baseUrl}
                    onInput={(e) => setDraft({ ...d(), baseUrl: e.currentTarget.value })}
                  />
                </label>
                <label class="field">
                  Model
                  <input
                    class="mono"
                    value={d().model}
                    onInput={(e) => setDraft({ ...d(), model: e.currentTarget.value })}
                  />
                </label>
                <label class="field">
                  API key
                  <input
                    type="password"
                    value={keyInput()}
                    placeholder={
                      d().hasKey ? "Key is set — enter a new one to change it" : "No key set"
                    }
                    onInput={(e) => setKeyInput(e.currentTarget.value)}
                  />
                </label>
                <Show when={PROVIDER_PRESETS.find((p) => p.label === d().label)?.keyHint}>
                  {(hint) => <p class="muted small">{hint()}</p>}
                </Show>

                <button
                  type="button"
                  class="ghost small"
                  onClick={() => setAdvancedOpen((v) => !v)}
                >
                  {advancedOpen() ? "Hide advanced" : "Show advanced"}
                </button>
                <Show when={advancedOpen()}>
                  <label class="field">
                    Chunk size
                    <input
                      type="number"
                      min="1"
                      value={d().chunkSize}
                      onInput={(e) =>
                        setDraft({ ...d(), chunkSize: Number(e.currentTarget.value) })
                      }
                    />
                  </label>
                  <label class="field">
                    Concurrency
                    <input
                      type="number"
                      min="1"
                      value={d().concurrency}
                      onInput={(e) =>
                        setDraft({ ...d(), concurrency: Number(e.currentTarget.value) })
                      }
                    />
                  </label>
                  <label class="field">
                    Max name length
                    <input
                      type="number"
                      min="1"
                      value={d().maxLen}
                      onInput={(e) => setDraft({ ...d(), maxLen: Number(e.currentTarget.value) })}
                    />
                  </label>
                  <label class="field">
                    Timeout (seconds)
                    <input
                      type="number"
                      min="1"
                      value={d().timeoutSecs}
                      onInput={(e) =>
                        setDraft({ ...d(), timeoutSecs: Number(e.currentTarget.value) })
                      }
                    />
                  </label>
                </Show>

                <div class="settings-actions">
                  <button type="button" class="primary" onClick={save}>
                    Save
                  </button>
                  <button
                    type="button"
                    class="ghost"
                    disabled={!d().hasKey}
                    onClick={() => s.clearApiKey(d().id)}
                  >
                    Clear key
                  </button>
                  <button
                    type="button"
                    class="ghost"
                    onClick={() => testConn(d().id)}
                    disabled={testing()}
                  >
                    {testing() ? "Testing…" : "Test connection"}
                  </button>
                  <Show when={s.settings().activeProfileId !== d().id}>
                    <button type="button" class="ghost" onClick={() => s.setActiveProfile(d().id)}>
                      Set active
                    </button>
                  </Show>
                  <button
                    type="button"
                    class="icon danger"
                    onClick={() => remove(d().id)}
                    title="Delete profile"
                  >
                    ✕
                  </button>
                </div>

                <Show when={testStatus()}>
                  {(status) => (
                    <p classList={{ "test-ok": status().ok, "test-error": !status().ok }}>
                      {status().ok ? "Connection ok." : `Failed: ${status().message}`}
                    </p>
                  )}
                </Show>
              </>
            )}
          </Show>
        </div>
      </aside>
    </Show>
  );
}
