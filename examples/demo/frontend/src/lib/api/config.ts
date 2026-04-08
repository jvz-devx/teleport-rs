import { configure } from '@teleport-rs/client';

configure({
	baseUrl: 'http://localhost:3000',
	timeout: 10_000,
	credentials: 'include',
});
