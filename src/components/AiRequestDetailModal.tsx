import { For, Show, createMemo } from "solid-js";

import * as s from "../store";
import { aiStatusBadgeVariant, aiStatusLabel } from "../lib/aiHistoryStatus";
import type { AiChunkDetail } from "../lib/types";
import { Badge, Button, Overlay } from "./common";

function AiChunkRow(props: { chunk: AiChunkDetail }) {
  return (
    <details class="ai-chunk-row" open={!!props.chunk.error}>
      <summary class="ai-chunk-summary">
        <span>Chunk {props.chunk.chunkIndex + 1}</span>
        <span class="muted small">
          {props.chunk.fileCount} file{props.chunk.fileCount === 1 ? "" : "s"} ·{" "}
          {props.chunk.elapsedMs}ms
        </span>
        <Show
          when={!props.chunk.error}
          fallback={
            <Badge variant="conflict" title={props.chunk.error ?? undefined}>
              Failed
            </Badge>
          }
        >
          <Badge variant="changed">OK</Badge>
        </Show>
      </summary>
      <div class="ai-chunk-body">
        <Show when={props.chunk.error}>
          <p class="step-error">{props.chunk.error}</p>
        </Show>
        <Show when={props.chunk.parsePath}>
          <p class="muted small">
            parse={props.chunk.parsePath} · model_returned={props.chunk.modelCount ?? "?"} ·
            dropped_unknown={props.chunk.droppedUnknown ?? 0} · sanitized=
            {props.chunk.sanitizedCount ?? 0} · missing={props.chunk.missingIds.length}
          </p>
        </Show>
        <div class="ai-detail-block">
          <span class="ai-detail-label">User prompt</span>
          <pre class="ai-detail-pre">{props.chunk.userPrompt}</pre>
        </div>
        <div class="ai-detail-block">
          <span class="ai-detail-label">Raw response</span>
          <pre class="ai-detail-pre">{props.chunk.rawResponse ?? "(no response received)"}</pre>
        </div>
      </div>
    </details>
  );
}

export function AiRequestDetailModal() {
  const id = () => s.aiDetailOpenId();
  const detail = createMemo(() => {
    const genId = id();
    return genId ? s.aiDetail().get(genId) : undefined;
  });

  return (
    <Show when={id()}>
      {(genId) => (
        <>
          <Overlay ariaLabel="Close AI request details" onClick={() => s.closeAiDetail()} />
          <div class="history-modal ai-detail-modal" role="dialog" aria-label="AI request details">
            <div class="history-modal-head">
              <div class="history-info">
                <span class="history-summary">AI Request</span>
                <span class="muted small mono">{genId()}</span>
              </div>
              <Button variant="icon" onClick={() => s.closeAiDetail()} title="Close">
                ✕
              </Button>
            </div>

            <Show
              when={detail()}
              fallback={
                <p class="muted small">
                  {s.isAiDetailLoading(genId())
                    ? "Loading…"
                    : "Generation still in progress — this will appear once it completes."}
                </p>
              }
            >
              {(d) => (
                <div class="ai-detail-body">
                  <div class="ai-detail-head-row">
                    <span class="muted small">{new Date(d().createdAt).toLocaleString()}</span>
                    <Badge variant={aiStatusBadgeVariant(d().status)}>
                      {aiStatusLabel(d().status)}
                    </Badge>
                    <Show when={d().mock}>
                      <Badge variant="warn" title="Simulated by the Dev menu's Mock AI toggle">
                        Mocked
                      </Badge>
                    </Show>
                  </div>

                  <Show when={d().error}>
                    <p class="step-error">{d().error}</p>
                  </Show>
                  <Show when={d().warning}>
                    <p class="step-error">{d().warning}</p>
                  </Show>

                  <div class="ai-detail-config">
                    <span>
                      <strong>Provider:</strong> {d().profileLabel} ({d().model})
                    </span>
                    <span>
                      <strong>Endpoint:</strong> {d().baseUrl}/chat/completions
                    </span>
                    <span>
                      <strong>Temperature:</strong> {d().temperature}
                    </span>
                    <span>
                      <strong>Files:</strong> {d().entryCount}
                    </span>
                    <span>
                      <strong>Chunks:</strong> {d().totalChunks} ({d().failedChunks} failed)
                    </span>
                    <span>
                      <strong>Chunk size / concurrency:</strong> {d().chunkSize} / {d().concurrency}
                    </span>
                    <span>
                      <strong>Timeout / max len:</strong> {d().timeoutSecs}s / {d().maxLen}
                    </span>
                    <span>
                      <strong>Has key:</strong> {d().hasKey ? "yes" : "no"}
                    </span>
                  </div>

                  <div class="ai-detail-block">
                    <span class="ai-detail-label">Instruction</span>
                    <pre class="ai-detail-pre">{d().instruction}</pre>
                  </div>

                  <details class="ai-detail-block">
                    <summary class="ai-detail-label">System prompt</summary>
                    <pre class="ai-detail-pre">{d().systemPrompt}</pre>
                  </details>

                  <div class="ai-chunk-list">
                    <For each={d().chunks}>{(chunk) => <AiChunkRow chunk={chunk} />}</For>
                  </div>
                </div>
              )}
            </Show>
          </div>
        </>
      )}
    </Show>
  );
}
