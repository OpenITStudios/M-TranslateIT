<script lang="ts">
  import markUrl from "./translateit-mark.svg";

  let { size = 36 }: { size?: number } = $props();
</script>

<img src={markUrl} alt="" aria-hidden="true" draggable="false" width={size} height={size} class="block shrink-0" />
