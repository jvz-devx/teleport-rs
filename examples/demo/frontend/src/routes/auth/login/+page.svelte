<script lang="ts">
	import { login } from '$lib/server/data.remote';
	import { goto } from '$app/navigation';

	let email = $state('');
	let password = $state('');
	let error = $state('');
	let loading = $state(false);

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		loading = true;

		try {
			await login({ email, password });
			await goto('/profile');
		} catch (err) {
			error = err instanceof Error ? err.message : 'Login failed';
		} finally {
			loading = false;
		}
	}
</script>

<h1>Login</h1>

{#if error}
	<p class="error">{error}</p>
{/if}

<form onsubmit={handleSubmit}>
	<label>
		Email
		<input type="email" bind:value={email} required />
	</label>

	<label>
		Password
		<input type="password" bind:value={password} required />
	</label>

	<button type="submit" disabled={loading}>
		{loading ? 'Logging in...' : 'Log in'}
	</button>
</form>

<style>
	.error {
		color: red;
	}
</style>
