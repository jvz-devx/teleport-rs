// Server-only helpers around the generated teleport-rs client.
//
// These functions keep the demo's SvelteKit routes small while showing the
// normal server-side integration: call Rust procedures through the generated
// TypeScript client and branch on typed Rust errors in one place.

import { api } from '$lib/api';
import { rpcUnwrap, mapError } from '@teleport-rs/client';
import type { LoginRequest, User, CreatePostRequest } from '$lib/api';

export async function getUsers(): Promise<User[]> {
	return rpcUnwrap(await api.users.listUsers());
}

export async function getUser(id: string): Promise<User> {
	return mapError(await api.users.getUser({ id }), (error) => {
		if (error.type === 'Detail' && error.detail.user_not_found) {
			throw new Error('User not found');
		}
		throw new Error(error.type);
	});
}

export async function login(input: LoginRequest) {
	return mapError(await api.auth.login(input), (error) => {
		if (error.type === 'Detail' && error.detail.invalid_credentials) {
			throw new Error('Invalid email or password');
		}
		throw new Error('Login failed');
	});
}

export async function getMyProfile(): Promise<User | null> {
	return mapError(await api.auth.getProfile(), (error) => {
		if (error.type === 'Unauthorized') return null;
		throw new Error(error.type);
	});
}

export async function createPost(input: CreatePostRequest) {
	return rpcUnwrap(await api.posts.createPost(input));
}
