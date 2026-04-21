import type { PageServerLoad } from './$types';
import { getUsers } from '$lib/server/data.remote';

export const load: PageServerLoad = async () => {
	return {
		users: await getUsers(),
	};
};
