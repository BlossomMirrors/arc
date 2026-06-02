<script lang="ts">
	import { authClient } from '$lib/auth-client';
	import { browser } from '$app/environment';

	if (browser) {
		authClient.signIn.oauth2({
			providerId: 'blossom-accounts',
			callbackURL: '/dashboard'
		});
	}
</script>
