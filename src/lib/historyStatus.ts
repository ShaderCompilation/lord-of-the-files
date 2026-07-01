// Shared display helpers for history badges/tooltips — used by both the History list
// and the operation detail modal, kept separate to avoid a circular import between them.

import type { CheckStatus, Operation } from "./types";

export function opBadgeVariant(status: Operation["status"]): "changed" | "unchanged" | "warn" {
  if (status === "applied") return "changed";
  if (status === "undone") return "unchanged";
  return "warn";
}

export function opStatusLabel(status: Operation["status"]): string {
  if (status === "applied") return "Applied";
  if (status === "undone") return "Undone";
  return "Partial";
}

export function rowBadgeVariant(status: "applied" | "undone"): "changed" | "unchanged" {
  return status === "applied" ? "changed" : "unchanged";
}

export function rowTooltip(status: "applied" | "undone"): string {
  return status === "applied"
    ? "This file is currently at its renamed location."
    : "This file has been reverted to its original name.";
}

export function checkBadgeVariant(status: CheckStatus): "changed" | "invalid" | "warn" {
  if (status === "ok") return "changed";
  if (status === "missing") return "invalid";
  return "warn";
}

export function checkLabel(status: CheckStatus): string {
  if (status === "ok") return "OK";
  if (status === "missing") return "Missing";
  return "Would overwrite";
}

export function checkTooltip(status: CheckStatus): string {
  if (status === "ok") return "This file can be moved as recorded.";
  if (status === "missing")
    return "This file no longer exists here — it may have been moved or deleted outside the app.";
  return "A different file already exists at the destination — proceeding could overwrite it.";
}

export function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** Trims the raw OS error suffix (e.g. "(os error 2)") for a cleaner inline message. */
export function cleanError(message: string): string {
  return message.replace(/\s*\(os error \d+\)\s*$/, "");
}
