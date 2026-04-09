<script lang="ts">
	import { getMyProfile } from '$lib/server/data.remote';
	import { onMount } from 'svelte';

	let profile = $state<{ name: string; email: string } | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);

	onMount(async () => {
		try {
			profile = await getMyProfile();
		} catch (err: unknown) {
			error = err instanceof Error ? err.message : 'Failed to load profile';
		} finally {
			loading = false;
		}
	});
</script>

<h1>Profile</h1>

{#if loading}
	<p>Loading profile...</p>
{:else if error}
	<p class="error">Error: {error}</p>
{:else if profile === null}
	<p>You are not logged in.</p>
	<a href="/auth/login">Go to login</a>
{:else}
	<dl>
		<dt>Name</dt>
		<dd>{profile.name}</dd>
		<dt>Email</dt>
		<dd>{profile.email}</dd>
	</dl>
{/if}

<style>
	.error {
		color: crimson;
	}
</style>
