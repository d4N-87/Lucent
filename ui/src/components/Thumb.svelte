<script lang="ts">
  import type { Layout, Lighting } from "../lib/api";
  import { toHex } from "../lib/color";
  import { look } from "../lib/look.svelte";

  let { layout, lighting }: { layout: Layout; lighting: Lighting } = $props();

  const GAP = 0.08;
</script>

<svg class="thumb" style:background={look.case} viewBox="-0.3 -0.3 {layout.width + 0.6} {layout.height + 0.6}" aria-hidden="true">
  {#each layout.keys as key (key.slot)}
    <rect
      x={key.x + GAP}
      y={key.y + GAP}
      width={key.w - 2 * GAP}
      height={key.h - 2 * GAP}
      rx="0.18"
      fill={toHex(lighting.overrides[key.slot] ?? lighting.background)}
    />
  {/each}
</svg>

<style>
  .thumb {
    display: block;
    width: 100%;
    height: auto;
    border-radius: 6px;
    box-shadow: inset 0 0 0 0.5px rgba(255, 255, 255, 0.1);
  }
</style>
