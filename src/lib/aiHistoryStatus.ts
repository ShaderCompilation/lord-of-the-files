// Shared display helpers for AI History badges — mirrors lib/historyStatus.ts's role for
// rename history, kept separate since the two domains (renames vs. AI requests) have distinct
// status shapes.

import type { AiGenerationStatus } from "./types";

export function aiStatusBadgeVariant(status: AiGenerationStatus): "changed" | "warn" | "conflict" {
  if (status === "ok") return "changed";
  if (status === "partial") return "warn";
  return "conflict";
}

export function aiStatusLabel(status: AiGenerationStatus): string {
  if (status === "ok") return "OK";
  if (status === "partial") return "Partial";
  return "Failed";
}
