// Demonstrates the SvelteKit remote functions pattern with teleport-rs.
//
// Remote functions run on the server and are callable from Svelte components
// via the experimental `$app/server` imports. Each function validates its
// input with Zod and calls the Rust backend through the generated client.
//
// Note: teleport-rs is framework-agnostic — the generated client works with
// any TypeScript framework. SvelteKit remote functions are one integration pattern.

import { query, command, form } from '$app/server'; // SvelteKit experimental
import { z } from 'zod';
import { auth, users, posts } from '$lib/api/generated/client';
import { rpcUnwrap, mapError } from '@teleport-rs/client';

// --- Query pattern: read-only data fetching ---

export const getUsers = query(async () => {
	return rpcUnwrap(await users.listUsers());
});

export const getUser = query(z.string(), async (id) => {
	return mapError(await users.getUser(id), (error) => {
		if (error.type === 'NotFound') throw new Error('User not found');
		throw new Error(error.type);
	});
});

// --- Command pattern: mutations that return data ---

export const login = command(
	z.object({ email: z.string().email(), password: z.string().min(1) }),
	async (input) => {
		return mapError(await auth.login(input), (error) => {
			if (error.type === 'Detail' && error.detail.invalidCredentials) {
				throw new Error('Invalid email or password');
			}
			throw new Error('Login failed');
		});
	},
);

// --- Auth-gated query: server-side cookie forwarding ---

export const getMyProfile = query(async () => {
	// Cookies are forwarded automatically because we configured
	// `credentials: "include"` in src/lib/api/config.ts.
	return mapError(await auth.me(), (error) => {
		if (error.type === 'Unauthorized') return null;
		throw new Error(error.type);
	});
});

// --- Form pattern: progressive enhancement with FormData ---

export const createPost = form(
	z.object({ title: z.string().min(1), body: z.string().min(1) }),
	async (input) => {
		return rpcUnwrap(await posts.create(input));
	},
);
