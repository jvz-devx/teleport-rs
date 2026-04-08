<script lang="ts">
	import { getUsers } from '$lib/server/data.remote';
	import { onMount } from 'svelte';

	let users = $state<{ id: string; name: string; email: string }[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);

	onMount(async () => {
		try {
			users = await getUsers();
		} catch (err: unknown) {
			error = err instanceof Error ? err.message : 'Failed to load users';
		} finally {
			loading = false;
		}
	});
</script>

<h1>teleport-rs Demo</h1>
<p>Users fetched from a Rust backend via generated TypeScript client.</p>

{#if loading}
	<p>Loading users...</p>
{:else if error}
	<p class="error">Error: {error}</p>
{:else if users.length === 0}
	<p>No users found.</p>
{:else}
	<ul>
		{#each users as user (user.id)}
			<li>{user.name} ({user.email})</li>
		{/each}
	</ul>
{/if}

<style>
	.error {
		color: crimson;
	}
</style>
