# Arc

Arc is a Linux software manager that provides a unified interface for managing
both Flatpak and native system packages. It wraps Flatpak (via libflatpak) and
PackageKit behind a single D-Bus daemon, then exposes that daemon to a CLI tool
and a graphical frontend.

## Architecture

The project is organized as a Cargo workspace with four crates:

```
arc/
  crates/
    libarc/          Shared types and client library (D-Bus proxy)
    arc-daemon/      Background daemon (D-Bus service)
    arc-cli/         Command-line interface
    arc-frontend/    GUI application (Slint)
```

### libarc

Foundation crate that every other crate depends on. It serves two roles:

1. **Shared type definitions** -- `Package`, `Transaction`, `TransactionStatus`,
   `TransactionType`, `Provider`, the `ArcError` error enum, and the `ArcEvent`
   enum used for signaling state changes. These live in the `types`, `errors`,
   and `events` modules and are re-exported from the crate root.

2. **D-Bus client library** -- Provides `ArcDaemonProxy`, a zbus-generated
   proxy for the `dev.arc.ArcDaemon1` interface. The public API is a single
   `connect()` function that returns a ready-to-use proxy over the session bus.
   Both the CLI and the frontend depend on this to talk to the daemon.

### arc-daemon

Long-running background process that registers itself on the session D-Bus as
`dev.arc.ArcDaemon1`. It owns two package providers and a transaction manager:

- **FlatpakProvider** -- Uses `libflatpak` for install/remove/update operations
  and shells out to `flatpak search` for search queries. Scans both user and
  system installations.
- **PackageKitProvider** -- Communicates with the system PackageKit daemon over
  the system D-Bus (`org.freedesktop.PackageKit`). Handles native distro
  packages.
- **MultiProvider** -- Wraps both providers behind the `PackageProvider` trait.
  Search, list-installed, and list-updates queries fan out to both providers in
  parallel and merge the results. Mutating operations (install, remove, update)
  are routed to the appropriate provider based on the package ID format.
- **TransactionManager** -- Tracks in-flight and completed operations as
  `Transaction` objects keyed by UUID. The daemon emits D-Bus signals
  (`TransactionStarted`, `TransactionProgress`, `TransactionFinished`,
  `UpdatesAvailable`) so clients can follow progress asynchronously.

### arc-cli

Command-line tool installed as `arc`. Built with clap, it supports:

- `arc install <app_id>` -- Install a package.
- `arc remove <app_id>` -- Remove a package.
- `arc update` -- List and apply all available updates.
- `arc search <query>` -- Search across all providers.
- `arc list` -- List installed applications.

Mutating commands poll the daemon for transaction progress and print a live
status line until the operation completes or fails.

### arc-frontend

Graphical application built with Slint. It connects to the daemon at startup
(gracefully degrading with a warning if the daemon is unavailable), loads the
installed application list, and presents a searchable package list with
install/remove buttons and a progress indicator. Async D-Bus calls run on a
Tokio runtime alongside the Slint event loop.

## Communication Flow

```
 arc-cli / arc-frontend
        |
        |  (session D-Bus, dev.arc.ArcDaemon1)
        v
    arc-daemon
        |
        +---> FlatpakProvider  ---> libflatpak / flatpak CLI
        |
        +---> PackageKitProvider ---> org.freedesktop.PackageKit (system D-Bus)
```

All client-daemon communication happens over the session bus using JSON-serialized
payloads. The daemon performs the actual privileged package operations and reports
results back through return values and D-Bus signals.

## Building

You will need:

- A Rust toolchain (stable, edition 2021)
- Development headers for Flatpak (`libflatpak-dev` or equivalent)
- A running PackageKit service (for native package support)
- D-Bus session bus

```
cargo build --release
```

The workspace produces three binaries:

| Binary         | Description          |
|----------------|----------------------|
| `arc-daemon`   | Background daemon    |
| `arc`          | CLI tool             |
| `arc-frontend` | Graphical frontend   |

## Running

Start the daemon first, then use either client:

```
# Terminal 1
./target/release/arc-daemon

# Terminal 2 (CLI)
./target/release/arc search firefox
./target/release/arc install org.mozilla.firefox

# Or launch the GUI
./target/release/arc-frontend
```

The daemon logs can be controlled with the `RUST_LOG` environment variable
(e.g. `RUST_LOG=debug`).

## License

See the project license file for details.