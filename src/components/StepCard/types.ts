import type { StepConfig } from "../../lib/types";

/** Narrow `props.step` to a specific variant for typed field access. */
export type Variant<T extends StepConfig["type"]> = Extract<StepConfig, { type: T }>;

export type SetFn = (patch: Record<string, unknown>) => void;
