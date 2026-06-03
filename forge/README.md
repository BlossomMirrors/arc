# Forge

Admin dashboard for the Arc PWA store. Manages apps, the Lutris whitelist, and front-page content, and exposes a public HTTP API consumed by the store client.

## Stack

- [SvelteKit](https://kit.svelte.dev/) + Svelte 5
- [Prisma](https://www.prisma.io/) on PostgreSQL
- [better-auth](https://www.better-auth.com/) with Authentik OIDC
- Tailwind CSS + shadcn-svelte

## Prerequisites

- Node.js >= 20 or Bun
- PostgreSQL database
- An [Authentik](https://goauthentik.io/) instance with an OAuth2/OIDC provider configured for this app (application slug: `arc-forge`)

## Setup

```sh
# Install dependencies
bun install

# Copy and fill in environment variables
cp .env.example .env

# Run database migrations
bunx prisma migrate deploy

# Start the dev server
bun run dev
```

## Environment variables

| Variable | Description |
|---|---|
| `DATABASE_URL` | PostgreSQL connection string, e.g. `postgresql://user:pass@localhost:5432/forge` |
| `BETTER_AUTH_SECRET` | Random secret used to sign sessions |
| `BETTER_AUTH_URL` | Public URL of this app, e.g. `https://forge.example.com` |
| `AUTHENTIK_URL` | Base URL of your Authentik instance, e.g. `https://auth.example.com` |
| `AUTHENTIK_CLIENT_ID` | OAuth client ID from the Authentik provider |
| `AUTHENTIK_CLIENT_SECRET` | OAuth client secret from the Authentik provider |

## Building

```sh
bun run build
bun run preview
```

## Docker

```sh
# Copy and fill in your .env, then:
docker compose up --build
```

The app starts on port 3000 and runs `prisma migrate deploy` automatically on startup.

## Dashboard

The dashboard at `/dashboard` is protected by Authentik login and provides:

- **PWAs**: create, edit, and delete Progressive Web App entries
- **Whitelist**: manage the Lutris game whitelist
- **Front Page**: visual block editor for the store's home page (type `/` anywhere to insert blocks)

## API

See [API.md](./API.md) for the public HTTP API consumed by store clients.
