import { createSignal } from "solid-js";

import { PROVIDER_PRESETS } from "../../lib/providers";
import type { ProviderProfile } from "../../lib/types";
import * as s from "../../store";

type TestStatus = { ok: boolean; message: string };

function blankProfile(): ProviderProfile {
  return {
    id: crypto.randomUUID(),
    label: "",
    baseUrl: "",
    model: "",
    chunkSize: 40,
    concurrency: 3,
    maxLen: 300,
    timeoutSecs: 60,
    hasKey: false,
  };
}

// Which preset a saved profile came from (matched by base URL); "Custom…" when nothing matches.
function matchPreset(baseUrl: string): string {
  const preset = PROVIDER_PRESETS.find((p) => p.baseUrl !== "" && p.baseUrl === baseUrl);
  return preset ? preset.label : "Custom…";
}

/**
 * Owns the draft/key/test state and the save/test logic for one provider profile.
 * Lives in the panel shell so navigation can seed it via `begin()` before showing the form;
 * the form and its steps stay presentational and read from it reactively.
 */
export interface ProfileEditor {
  draft: () => ProviderProfile | null;
  update: (patch: Partial<ProviderProfile>) => void;
  selectedProvider: () => string | null;
  selectPreset: (label: string) => void;
  isCustom: () => boolean;
  keyHint: () => string | undefined;
  nickPlaceholder: () => string;
  keyInput: () => string;
  setKeyInput: (value: string) => void;
  replacingKey: () => boolean;
  setReplacingKey: (value: boolean) => void;
  removeKey: () => Promise<void>;
  advancedOpen: () => boolean;
  toggleAdvanced: () => void;
  canSave: () => boolean;
  save: () => Promise<boolean>;
  testing: () => boolean;
  testStatus: () => TestStatus | null;
  test: () => Promise<void>;
  /** Seed the editor for adding (null) or editing an existing profile. */
  begin: (profile: ProviderProfile | null) => void;
}

export function createProfileEditor(): ProfileEditor {
  const [draft, setDraft] = createSignal<ProviderProfile | null>(null);
  const [selectedProvider, setSelectedProvider] = createSignal<string | null>(null);
  const [keyInput, setKeyInput] = createSignal("");
  const [replacingKey, setReplacingKey] = createSignal(false);
  const [advancedOpen, setAdvancedOpen] = createSignal(false);
  const [testStatus, setTestStatus] = createSignal<TestStatus | null>(null);
  const [testing, setTesting] = createSignal(false);

  const isCustom = () => selectedProvider() === "Custom…";
  const keyHint = () => PROVIDER_PRESETS.find((p) => p.label === selectedProvider())?.keyHint;
  const nickPlaceholder = () => {
    const sp = selectedProvider();
    return sp && sp !== "Custom…" ? sp : "My provider";
  };
  const canSave = () => {
    const d = draft();
    return !!d && selectedProvider() !== null && d.model.trim() !== "" && d.baseUrl.trim() !== "";
  };

  function update(patch: Partial<ProviderProfile>) {
    const d = draft();
    if (d) setDraft({ ...d, ...patch });
  }

  function begin(profile: ProviderProfile | null) {
    setDraft(profile ? { ...profile } : blankProfile());
    setSelectedProvider(profile ? matchPreset(profile.baseUrl) : null);
    setKeyInput("");
    setReplacingKey(false);
    setAdvancedOpen(false);
    setTestStatus(null);
  }

  // Pick a provider tile: prefill base URL + model, and the nickname unless the user set a custom one.
  function selectPreset(label: string) {
    const preset = PROVIDER_PRESETS.find((p) => p.label === label);
    const d = draft();
    if (!preset || !d) return;
    const isDefaultLabel = d.label === "" || PROVIDER_PRESETS.some((p) => p.label === d.label);
    setSelectedProvider(label);
    setDraft({
      ...d,
      baseUrl: preset.baseUrl,
      model: preset.defaultModel,
      label:
        label === "Custom…"
          ? isDefaultLabel
            ? ""
            : d.label
          : isDefaultLabel
            ? preset.label
            : d.label,
    });
  }

  async function save(): Promise<boolean> {
    const d = draft();
    if (!d || !canSave()) return false;
    await s.upsertProfile(d);
    const hasKeyInput = keyInput().trim() !== "";
    if (hasKeyInput) await s.saveApiKey(d.id, keyInput());
    // First profile ever (or none active after a delete): make it active so AI Rename works right away.
    if (!s.settings().activeProfileId) await s.setActiveProfile(d.id);
    if (hasKeyInput) {
      setDraft({ ...d, hasKey: true });
      setKeyInput("");
      setReplacingKey(false);
    }
    return true;
  }

  async function test() {
    const d = draft();
    if (!d) return;
    setTesting(true);
    setTestStatus(null);
    try {
      // Auto-save first so we test exactly what's on screen (test_connection loads the saved profile).
      if (!(await save())) return;
      await s.testConnection(d.id);
      setTestStatus({ ok: true, message: "ok" });
    } catch (e) {
      setTestStatus({ ok: false, message: String(e) });
    } finally {
      setTesting(false);
    }
  }

  async function removeKey() {
    const d = draft();
    if (!d) return;
    await s.clearApiKey(d.id);
    setDraft({ ...d, hasKey: false });
    setReplacingKey(false);
    setKeyInput("");
  }

  return {
    draft,
    update,
    selectedProvider,
    selectPreset,
    isCustom,
    keyHint,
    nickPlaceholder,
    keyInput,
    setKeyInput,
    replacingKey,
    setReplacingKey,
    removeKey,
    advancedOpen,
    toggleAdvanced: () => setAdvancedOpen((v) => !v),
    canSave,
    save,
    testing,
    testStatus,
    test,
    begin,
  };
}
