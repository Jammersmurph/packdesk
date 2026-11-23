<script lang="ts">
	import icon from '@/assets/packdesk.svg'
	import { app } from '@/lib/state/app.svelte'
	import { openExternal, openFolder } from '@/lib/tauri/windows'
	import { shortenHome } from '@/lib/util/path'
</script>

<div class="about">
	<img src={icon} alt="" width="72" height="72" />
	<h1 class="title-doc">Packdesk</h1>
	<span class="mono">{app.info?.version ?? ''}</span>
	<p>
		A Git-native desktop workspace for packwiz. Modified from Packweave on November 23, 2026.
		Released under GPLv3, without warranty. You may share and modify it under that license.
	</p>
	<div class="links">
		<button
			class="btn"
			onclick={() => void openExternal('https://github.com/Jammersmurph/packdesk')}
			>Source code</button
		>
		<button class="btn" onclick={() => void openExternal('https://github.com/Jammersmurph/packdesk/blob/main/LICENSE')}>License</button>
		<button class="btn" onclick={() => void openExternal('https://github.com/packweavers/packweave')}>Upstream Packweave</button>
		<button class="btn" onclick={() => app.info && void openFolder(app.info.dataDir)}
			>Data folder</button
		>
	</div>
	{#if app.info}<span class="small mono">{shortenHome(app.info.dataDir)}</span>{/if}
</div>

<style>
	.about {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		padding: 48px 24px;
		text-align: center;
	}

	.links {
		display: flex;
		gap: 1px;
		margin-top: 8px;
	}
</style>
