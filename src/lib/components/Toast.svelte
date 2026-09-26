<script lang="ts">
  import { toast } from '$lib/toast.svelte';
</script>

<div class="toast-area" data-testid="toast-area">
  {#each toast.items as t (t.id)}
    <button
      class="toast {t.kind}"
      data-testid="toast-item"
      onclick={() => toast.dismiss(t.id)}
    >{t.text}</button>
  {/each}
</div>
