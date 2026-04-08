// Demonstrates the SvelteKit remote functions pattern with teleport-rs.
//
// Remote functions run on the server and are callable from Svelte components
// via the experimental `$app/server` imports. Each function validates its
// input with Zod and calls the Rust backend through the generated client.

import { query, command, form } from '$app/server'; // SvelteKit experimental
import { z } from 'zod';
import { auth, users, posts } from '$lib/api/generated/client';
import { isTransportError } from '@teleport-rs/client';

// --- Query pattern: read-only data fetching ---

export const getUsers = query(async () => {
	const result = await users.listUsers();
	if (!result.ok) {
		if (isTransportError(result)) throw new Error(result.transport.message);
		throw new Error(result.error.type);
	}
	return result.data;
});

export const getUser = query(z.string(), async (id) => {
	const result = await users.getUser(id);
	if (!result.ok) {
		if (isTransportError(result)) throw new Error(result.transport.message);
		if (result.error.type === 'NotFound') throw new Error('User not found');
		throw new Error(result.error.type);
	}
	return result.data;
});

// --- Command pattern: mutations that return data ---

export const login = command(
	z.object({ email: z.string().email(), password: z.string().min(1) }),
	async (input) => {
		const result = await auth.login(input);
		if (!result.ok) {
			if (isTransportError(result)) throw new Error(result.transport.message);
			if (result.error.type === 'Detail') {
				if (result.error.detail.invalidCredentials) {
					throw new Error('Invalid email or password');
				}
			}
			throw new Error('Login failed');
		}
		return result.data;
	},
);

// --- Auth-gated query: server-side cookie forwarding ---

export const getMyProfile = query(async () => {
	// Cookies are forwarded automatically because we configured
	// `credentials: "include"` in src/lib/api/config.ts.
	const result = await auth.me();
	if (!result.ok) {
		if (isTransportError(result)) throw new Error(result.transport.message);
		if (result.error.type === 'Unauthorized') return null;
		throw new Error(result.error.type);
	}
	return result.data;
});

// --- Form pattern: progressive enhancement with FormData ---

export const createPost = form(
	z.object({ title: z.string().min(1), body: z.string().min(1) }),
	async (input) => {
		const result = await posts.create(input);
		if (!result.ok) {
			if (isTransportError(result)) throw new Error(result.transport.message);
			throw new Error(result.error.type);
		}
		return result.data;
	},
);
