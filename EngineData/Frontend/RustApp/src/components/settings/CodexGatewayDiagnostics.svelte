<script lang="ts">
  import { onMount } from "svelte";
  import { codexGatewayApi, type CodexGatewayStatus } from "../../app/bridge/codexGatewayApi";

  let status = $state<CodexGatewayStatus | null>(null);
  let busy = $state(false);
  let closed = false;

  async function perform(action: "status" | "connect" | "disconnect"): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      const result = await codexGatewayApi[action]();
      if (!closed) status = result;
    } finally {
      if (!closed) busy = false;
    }
  }

  onMount(() => {
    closed = false;
    void perform("status");
    return () => { closed = true; };
  });

  const connected = $derived(status?.process_owned === true);
  const stopping = $derived(status?.state === "stopping" || status?.state === "cleanup_incomplete");
</script>

<article class="ti-panel p-5">
  <div>
    <span class="ti-field-label">Experimental connection diagnostics</span>
    <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Codex App Server</h4>
    <p class="mb-0 mt-2 text-[12px] leading-5 text-[var(--ti-text-muted)]">
      Optional ChatGPT account connectivity check only. Codex audio translation is not enabled for Meeting.
      The existing local engine remains the active translation route.
    </p>
  </div>
  <div class="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-3">
    <div class="ti-state-card">
      <span class="ti-field-label">Connection</span>
      <strong class="mt-2 block text-[12px]">{status?.state ?? "Not checked"}</strong>
    </div>
    <div class="ti-state-card">
      <span class="ti-field-label">Account</span>
      <strong class="mt-2 block text-[12px]">{status?.chatgpt_authenticated ? "ChatGPT confirmed" : "Not verified"}</strong>
    </div>
    <div class="ti-state-card">
      <span class="ti-field-label">Included usage</span>
      <strong class="mt-2 block text-[12px]">{status?.included_usage_allowed ? "Permitted" : "Not verified / unavailable"}</strong>
    </div>
  </div>
  <p class="mb-0 mt-3 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
    {status?.blocker || "No diagnostic blocker reported."}
    Voice entitlement, audio input, translated output, and zero-additional-charge admission are not certified by this check.
  </p>
  <div class="ti-action-row mt-4">
    <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void perform("status")}>
      {busy ? "Checking..." : "Refresh"}
    </button>
    {#if connected || stopping}
      <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void perform("disconnect")}>
        Disconnect owned App Server
      </button>
    {:else}
      <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void perform("connect")}>
        Check Codex connection
      </button>
    {/if}
  </div>
  <p class="mb-0 mt-3 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
    This check starts an owned local Codex App Server only on request. It does not sign you in,
    initiate a model turn, consume realtime audio, or switch the Meeting engine.
  </p>
</article>
