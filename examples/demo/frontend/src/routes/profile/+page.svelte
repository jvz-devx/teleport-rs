<script lang="ts">
	import type { User } from '$lib/api';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const profile = $derived(data.profile as Pick<User, 'name' | 'email'> | null);
</script>

<h1>Profile</h1>

{#if profile === null}
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
</style>
