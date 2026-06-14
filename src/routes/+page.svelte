<script lang="ts">
  import { onMount } from "svelte";
  import { mode, banner, boot, sessions, selectedSessionId } from "$lib/stores";
  import { SAMPLE_SESSIONS } from "$lib/mock";
  import { isTauri } from "$lib/ipc";

  import Onboarding from "$lib/screens/Onboarding.svelte";
  import Dashboard from "$lib/screens/Dashboard.svelte";
  import NewSession from "$lib/screens/NewSession.svelte";
  import Live from "$lib/screens/Live.svelte";
  import Post from "$lib/screens/Post.svelte";
  import Settings from "$lib/screens/Settings.svelte";

  onMount(() => {
    // In a plain-browser preview (no Tauri), seed sample data so the dashboard
    // is populated. Inside Tauri, boot() loads the real list from disk.
    if (!isTauri()) {
      sessions.set(SAMPLE_SESSIONS);
      selectedSessionId.set(SAMPLE_SESSIONS[0].id);
    }
    void boot();
  });
</script>

<div class="app">
  <div class="titlebar">
    <div class="lights">
      <span class="light l-c"></span><span class="light l-m"></span><span class="light l-x"></span>
    </div>
    <div class="tb-title"><b>Call&nbsp;Assistant</b></div>
  </div>

  {#if $banner}
    <div class="banner">
      <span>{$banner}</span>
      <button class="banner-x" aria-label="Dismiss" onclick={() => banner.set(null)}>✕</button>
    </div>
  {/if}

  <div class="stage">
    {#if $mode === "booting"}
      <section class="screen boot">
        <div class="boot-inner">Loading…</div>
      </section>
    {:else if $mode === "onboarding"}
      <Onboarding />
    {:else if $mode === "dashboard"}
      <Dashboard />
    {:else if $mode === "new"}
      <NewSession />
    {:else if $mode === "live"}
      <Live />
    {:else if $mode === "post"}
      <Post />
    {:else if $mode === "settings"}
      <Settings />
    {/if}
  </div>
</div>

<style>
  .banner{
    flex:none;display:flex;align-items:center;gap:14px;
    padding:10px 18px;font-size:12.5px;color:var(--late);
    background:rgba(255,107,92,.1);border-bottom:1px solid rgba(255,107,92,.25);
  }
  .banner span{flex:1}
  .banner-x{background:none;border:0;color:var(--late);cursor:pointer;font-size:13px}
  .boot{align-items:center;justify-content:center}
  .boot-inner{font-family:var(--f-mono);font-size:12px;letter-spacing:.18em;text-transform:uppercase;color:var(--ink-3)}
</style>
