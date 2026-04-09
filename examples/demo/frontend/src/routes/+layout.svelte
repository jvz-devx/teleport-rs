<script lang="ts">
	import '$lib/api/config';
	import favicon from '$lib/assets/favicon.svg';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<nav>
	<a href="/">Home</a>
	<a href="/auth/login">Login</a>
	<a href="/profile">Profile</a>
</nav>

<main>
	{@render children()}
</main>
