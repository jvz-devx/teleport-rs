import { fail, redirect } from '@sveltejs/kit';
import type { Actions } from './$types';
import { login } from '$lib/server/data.remote';

export const actions: Actions = {
	default: async ({ request }) => {
		const formData = await request.formData();
		const email = String(formData.get('email') ?? '');
		const password = String(formData.get('password') ?? '');

		try {
			await login({ email, password });
		} catch (error) {
			return fail(400, {
				email,
				error: error instanceof Error ? error.message : 'Login failed',
			});
		}

		throw redirect(303, '/profile');
	},
};
