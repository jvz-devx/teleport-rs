import type { PageServerLoad } from './$types';
import { getMyProfile } from '$lib/server/data.remote';

export const load: PageServerLoad = async () => {
	return {
		profile: await getMyProfile(),
	};
};
