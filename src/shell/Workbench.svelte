<script lang="ts">
	import { layout } from '@/lib/state/layout.svelte'
	import { panels } from '@/lib/state/panels.svelte'
	import { ws } from '@/lib/state/workspace.svelte'

	import ActivityBar from './ActivityBar.svelte'
	import Editor from './Editor.svelte'
	import Inspector from './Inspector.svelte'
	import Panel from './Panel.svelte'
	import Sidebar from './Sidebar.svelte'
	import StatusBar from './StatusBar.svelte'
</script>

<div class="body">
	<ActivityBar />
	{#if layout.sidebar}
		<Sidebar />
	{/if}
	<div class="center">
		{#if ws.nativePackwiz}
			<div class="native-banner">
				<strong>packwiz workspace · preview</strong>
				<span
					>Native TOML, content, files and Git are connected. Instance sync and automated publishing
					are not ready yet.</span
				>
			</div>
		{/if}
		<Editor />
		{#if layout.panel && panels.shown.length > 0}
			<Panel />
		{/if}
	</div>
	{#if layout.inspector}
		<Inspector />
	{/if}
</div>
<StatusBar />

<style>
	.native-banner {
		padding: 10px 14px;
		border-bottom: 1px solid var(--line);
		background: var(--band);
		font-size: 12px;
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
	}
	.body {
		flex: 1;
		min-height: 0;
		display: flex;
	}

	.center {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
</style>
