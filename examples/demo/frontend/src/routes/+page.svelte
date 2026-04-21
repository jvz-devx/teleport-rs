<script lang="ts">
	import type { User } from '$lib/api';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const users = $derived(data.users as User[]);
</script>

<h1>teleport-rs Demo</h1>
<p>Users fetched from a Rust backend via generated TypeScript client.</p>

{#if users.length === 0}
	<p>No users found.</p>
{:else}
	<ul>
		{#each users as user (user.id)}
			<li>{user.name} ({user.email})</li>
		{/each}
	</ul>
{/if}

<style>
</style>
