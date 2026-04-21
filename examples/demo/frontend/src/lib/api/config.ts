import { createClient } from '@teleport-rs/client';
import { bindClient } from './generated/client';

const client = createClient({
	baseUrl: 'http://localhost:3000',
	timeout: 10_000,
	credentials: 'include',
});

export const api = bindClient(client);
