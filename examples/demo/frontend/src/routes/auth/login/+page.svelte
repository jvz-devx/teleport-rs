<script lang="ts">
	import type { ActionData } from './$types';

	let { form }: { form: ActionData } = $props();
</script>

<h1>Login</h1>

{#if form?.error}
	<p class="error">{form.error}</p>
{/if}

<form method="POST">
	<label>
		Email
		<input type="email" name="email" value={form?.email ?? ''} required />
	</label>

	<label>
		Password
		<input type="password" name="password" required />
	</label>

	<button type="submit">Log in</button>
</form>

<style>
	.error {
		color: red;
	}
</style>
