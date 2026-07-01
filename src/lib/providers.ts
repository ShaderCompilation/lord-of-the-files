// Presets for the "one universal adapter: OpenAI-compatible Chat Completions" approach (see
// docs/byok-ai-rename-plan.md). Base URL + a default model are prefilled but user-editable —
// model ids drift, so treat these as a starting point, not a guarantee.

export interface ProviderPreset {
  label: string;
  baseUrl: string;
  defaultModel: string;
  keyHint?: string;
}
 
export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-4.1-mini",
  },
  {
    label: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    defaultModel: "deepseek/deepseek-v4-flash",
    keyHint: "One key, hundreds of models across every major provider.",
  },
  {
    label: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    defaultModel: "llama-3.3-70b-versatile",
  },
  {
    label: "Together",
    baseUrl: "https://api.together.xyz/v1",
    defaultModel: "deepseek-ai/DeepSeek-V4-Flash",
  },
  {
    label: "Fireworks",
    baseUrl: "https://api.fireworks.ai/inference/v1",
    defaultModel: "accounts/fireworks/models/deepseek-v4-flash",
  },
  {
    label: "DeepInfra",
    baseUrl: "https://api.deepinfra.com/v1/openai",
    defaultModel: "deepseek-ai/DeepSeek-V4-Flash",
  },
  {
    label: "Mistral",
    baseUrl: "https://api.mistral.ai/v1",
    defaultModel: "mistral-small-latest",
  },
  {
    label: "DeepSeek",
    baseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek-v4-flash",
  },
  {
    label: "xAI (Grok)",
    baseUrl: "https://api.x.ai/v1",
    defaultModel: "grok-4.1-fast",
  },
  {
    label: "Perplexity",
    baseUrl: "https://api.perplexity.ai",
    defaultModel: "sonar",
  },
  {
    label: "Gemini",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai/",
    defaultModel: "gemini-2.5-flash-lite",
  },
  {
    label: "Ollama (local)",
    baseUrl: "http://localhost:11434/v1",
    defaultModel: "llama3.1",
    keyHint: "Local server — leave the API key blank.",
  },
  {
    label: "LM Studio (local)",
    baseUrl: "http://localhost:1234/v1",
    defaultModel: "local-model",
    keyHint: "Local server — leave the API key blank.",
  },
  {
    label: "Custom…",
    baseUrl: "",
    defaultModel: "",
  },
];
