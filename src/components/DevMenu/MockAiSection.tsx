import * as s from "../../store";
import { Checkbox, Field, SelectField } from "../common";
import type { MockAiConfig, MockTransform } from "../../lib/types";

const TRANSFORMS: { value: MockTransform; label: string }[] = [
  { value: "suffix", label: 'Add "_mock" suffix' },
  { value: "uppercase", label: "UPPERCASE" },
  { value: "lowercase", label: "lowercase" },
  { value: "reverse", label: "esreveR" },
  { value: "slugify", label: "slugify-like-this" },
];

function update(patch: Partial<MockAiConfig>) {
  void s.setMockAiConfig({ ...s.settings().mockAi, ...patch });
}

export function MockAiSection() {
  const cfg = () => s.settings().mockAi;

  return (
    <section class="dev-section">
      <h3>Mock AI</h3>
      <p class="muted small hint">
        Simulates the AI rename backend — no network calls, no API cost, no waiting on a real
        provider. Runs through the same chunking/progress/reconcile pipeline as a real
        generation, just with fake results.
      </p>
      <Checkbox checked={cfg().enabled} onChange={(v) => update({ enabled: v })}>
        Enable Mock AI
      </Checkbox>
      <Field label="Simulated latency (ms)">
        <input
          type="number"
          min="0"
          value={cfg().latencyMs}
          onInput={(e) => update({ latencyMs: Math.max(0, Number(e.currentTarget.value)) })}
        />
      </Field>
      <Field label="Simulated failure rate (%)">
        <input
          type="number"
          min="0"
          max="100"
          value={Math.round(cfg().failRate * 100)}
          onInput={(e) => {
            const pct = Math.min(100, Math.max(0, Number(e.currentTarget.value)));
            update({ failRate: pct / 100 });
          }}
        />
      </Field>
      <SelectField
        label="Rename transform"
        value={cfg().transform}
        onChange={(v) => update({ transform: v as MockTransform })}
      >
        {TRANSFORMS.map((t) => (
          <option value={t.value}>{t.label}</option>
        ))}
      </SelectField>
    </section>
  );
}
