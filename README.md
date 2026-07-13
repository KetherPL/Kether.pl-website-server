# Kether.pl Website Server (Kether Internal Services Server)

This project is the backend server for the Kether.pl website, a homepage for the Polish community-driven [Left 4 Dead 2 ZoneMod-based server](https://github.com/Krevik/Kether.pl-L4D2-Server). It's built using Rust with a lightweight JSON-based database, providing REST API endpoints for binds, commands, voting, Steam integration, and live server information.

## Features

*   **L4D2 Server Information:**
    *   Retrieves and provides live information about L4D2 servers, including:
        *   Server name
        *   Current map
        *   Number of players and bots
        *   Player details (name, score, duration)
    *   Supports querying specific servers by IP and port.
    *   Includes a hardcoded endpoint for the Kether.pl L4D2 server.
*   **Steam Integration:**
    *   Fetches Steam user data (name, avatar, profile URL, etc.) using the Steam Web API (to display to the logged-in user on the website).
    *   Checks if a user owns Left 4 Dead 2 on their Steam account (a check for login purposes on our website).
*   **Steam Bot Integration:**
    *   The server includes a Steam bot that can log in to a Steam account and send messages to the selected Steam chat group channel.
    *   Uses the SC_Sub_Poster library for Steam chat functionality.
    *   Supports sending formatted messages with player mentions and group notifications.
    *   Thread-safe implementation with global access for message sending from other parts of the application.
    *   **Steam chat commands** (when `rest_call_for_sub` is enabled): mention the bot (or set `commands_without_mention = true`) and use `!command` syntax.
        *   **`!plan` / `!p`** — schedule a lobby time (CET/CEST; formats `19:00`, `18.30`, or `16`). Optional server `1`/`2`, optional map via `m`/`map` (e.g. `!plan 19:00 m No Mercy`). `!plan clear [1|2]` clears reservations. Supports per-server targeting, replan detection, and optional user mentions.
        *   **`!poll` / `!q`** — yes/no polls (thumb reactions) or multi-choice polls (`-o` options, up to 10, `:steamthis:` reactions on each choice). A single `-o` posts question + option without reactions.
        *   **`!status` / `!s`** — query configured L4D2 server(s) for live info (`server_query` feature). Optional `1`/`2` and `full`/`f` for detailed player lists.
        *   **`!help` / `!h`** — list available bot commands.
        *   **`!mute`** — admin-only: mute a user by mention; auto-deletes their messages globally until expiry. Duration: minutes (default 45), `h` for hours (e.g. `1:30` = 1h 30m; default 1), or `d` for days (default 1). Max duration configurable via `steam.chat.admin.mute_max_minutes` (default 7 days).
        *   **`!unmute`** — admin-only: remove a mute from a mentioned user.
        *   **`!lsmute`** — admin-only: list muted users and remaining time.
    *   **Mute persistence:** mutes up to 12 hours are kept in memory only; longer mutes are saved to `muted_users.json` beside the executable and restored on startup.
    *   **SteamBot admins:** `steam.chat.admin.admins_same_as_frontend` (default `true`) uses `frontend_admins`; set to `false` and configure `steam.chat.admin.admins` for a separate admin list.
    *   **Dedicated chat routing** for `!plan` and `!poll`: post successful responses to a separate chat room (`plan_chat_id` / `poll_chat_id`). Usage and validation errors always reply in the chat where the command was invoked.
    *   **Plan WebSocket broadcast:** `GET /api/ws/plan` pushes `SET <timestamp>` / `CLEAR` messages to connected L4D2 servers or other clients when reservations change.
*   **LiveServer Call for Subs:**
    *   The core functionality involves fetching and processing "call for sub" requests from a live L4D2 server (requires a [dedicated SourceMod plugin](https://github.com/Krevik/Kether.pl-L4D2-Server/blob/kether_2.0/addons/sourcemod/scripting/kether/l4d2_call_for_sub_rest.sp)).
    *   Automatically formats messages with player mentions and sends them to the configured Steam group chat.
    *   Integrates with the Steam Bot to handle message delivery.
*   **REST API Endpoint:**
    *   Provides a simple REST endpoint for the L4D2 game server to retrieve SteamID64 (via POST method data) and send Call For Sub requests.
    *   Endpoint: `POST /api/callForSub/`
    *   Accepts JSON payload with Steam ID and returns appropriate HTTP status codes.
*   **Steam API Integration:**
    *   It uses the Steam API to fetch player information, such as player names, based on SteamIDs.
    *   Enhanced integration for call-for-sub functionality with player profile lookup.
*   **Database Management:**
    *   Uses JSON files for lightweight, persistent data storage.
    *   In-memory database with atomic file-based persistence (no external database required).
    *   Manages the following data:
        *   **Binds:** Custom in-game keybinds with author, text, and vote tallies (voter Steam IDs kept internal; public GET returns counts only).
        *   **Bind Suggestions:** User-submitted bind suggestions.
        *   **Commands:** Server commands with descriptions.
        *   **Bind Votings:** User votes embedded directly in bind objects for data consistency.
    *   Provides RESTful API endpoints for CRUD (Create, Read, Update, Delete) operations on database entities.
    *   Thread-safe with RwLock for concurrent read access and exclusive write access.
*   **RESTful API:**
    *   Built with the Rocket web framework.
    *   Provides a comprehensive set of API endpoints for interacting with the server's features.
    *   Supports CORS (Cross-Origin Resource Sharing) to allow requests from the Kether.pl frontend (running on `localhost:3000` and `kether.pl`), and the L4D2 server.
*   **KetherServerDaemon maps bridge:**
    *   Receives a 1:1 copy of the daemon map registry via `POST /api/registry/sync` (Bearer token).
    *   Serves the public installed-maps list at `GET /api/maps` from the synced registry only (empty when never synced; stale cached list when daemon is unreachable). Static fallback lives on the frontend.
    *   Configure one shared secret in `[server_daemon].sync_api_key` and the daemon's `backend_api_key`; it authenticates both registry sync and admin install/manage calls.
    *   Point the daemon at `backend_api_url = "http://127.0.0.1:3001/api"` (website-server port).
*   **FastDL Server:**
    *   **HTTP File Server:** Serves static files for Source/GoldSrc games (maps, models, sounds, MOTDs) via HTTP.
    *   **Directory Listings:** Automatic HTML directory listings for folders without `index.html` files.
    *   **Dark Theme:** Modern dark-themed directory listings with hover effects and professional styling.
    *   **Smart Caching:** Configurable cache headers with 1-year caching for static content (`Cache-Control: public, max-age=31536000, immutable`).
    *   **Async File Operations:** Uses `smol` async runtime for efficient file system operations.
    *   **Security:** Path traversal protection and proper error handling.
    *   **Route Priority:** FileServer handles files first, custom catcher provides directory listings for folders.
    *   **ETag Support:** Automatic ETag generation for improved cache validation.
    *   **Accessible via:** `http://your-domain/fastdl/` (e.g., `/fastdl/l4d2_kether/resource/`)
*   **Security:**
    *   Redirects root path (`/`) to the Kether.pl frontend.
*   **Configuration:**
    *   Uses a `config.toml` configuration file with clean, nested structure:
        *   Steam Web API key
        *   Steam account credentials for bot functionality
        *   Steam group chat IDs for message delivery
        *   Dedicated plan/poll chat IDs and behavior flags (routing, cleanup, user mentions)
        *   Primary and secondary L4D2 server addresses (for `!status` and targeted `!plan`)
    *   **Hot reload:** When built with the `hot_reload` feature (included in the default `kether_meta` bundle), changes to `config.toml` are picked up automatically without restarting the process.
    *   JSON database files are automatically created in the executable directory:
        *   `cmds.json` - Server commands
        *   `binds.json` - User binds with embedded voting
        *   `bind_sgs.json` - Bind suggestions
*   **Interactive REPL:**
    *   While the daemon runs with `--service`, press **`C`** to open an in-process command console.
    *   Supports restarting or stopping the daemon without leaving the `screen`/terminal session.
    *   In-process restart reloads the REST server and SteamBot from disk (useful after changing bot credentials or chat IDs).
    *   **`chats` / `groups` / `G`** — list Steam chat groups and their chat rooms with IDs (uses the live SteamBot connection; requires `rest_call_for_sub`).
*   **Command-Line Interface (CLI):**
    *   Allows querying L4D2 servers directly from the command line.
    *   Starts the RESTful server service.
* **Error Handling:**
    * Robust error handling for database operations, Steam API calls, and game server queries.
    * Returns appropriate HTTP status codes (e.g., 404 Not Found, 400 Bad Request, 500 Internal Server Error) for API requests.
* **Logging:**
    * Logs database errors to the console for debugging.

## Dependencies

*   **Rocket:** Web framework for building the RESTful API with JSON serialization support.
*   **gamedig:** Library for querying game servers.
*   **steam-rs:** Library for interacting with the Steam Web API.
*   **SC_Sub_Poster:** Custom library for Steam chat functionality and bot integration.
*   **rocket_cors:** CORS support for the Rocket web server.
*   **clap:** Command-line argument parsing.
*   **toml:** Configuration file parsing (config.toml).
*   **colored:** Terminal output coloring.
*   **tokio:** Async runtime for concurrent operations.
*   **smol:** Async runtime for efficient file system operations in FastDL server.
*   **once_cell:** Global state management for Steam Bot integration.
*   **notify-debouncer-full:** File-system watching for config hot reload.
*   **reedline / crossterm:** Interactive REPL console when running as a service.

## Getting Started

1.  **Install Rust:** If you don't have Rust installed, follow the instructions on the official Rust website (https://www.rust-lang.org/tools/install).
2.  **Clone the Repository:**
    ```bash
    git clone <repository_url>
    cd Kether.pl-website-server
    ```
3.  **Configure `config.toml`:** Create a `config.toml` file in the project's root directory (or it will be created automatically on first run).
    *   The file will be auto-generated with helpful comments on first run
    *   **Steam Web API:**
        *   Get your API key from: https://steamcommunity.com/dev/apikey
        *   Set `steam.web_api_key = "YOUR_KEY_HERE"`
    *   **Steam Bot** (for !sub posting):
        *   Set `steam.bot.username = "your_steam_username"`
        *   Set `steam.bot.password = "your_steam_password"`
        *   Optional: `steam.chat.admin.admins_same_as_frontend = false` and `steam.chat.admin.admins = [...]` for a separate SteamBot admin list (`!mute`, `!unmute`, `!lsmute`)
        *   Optional: `steam.chat.admin.mute_max_minutes` (default 10080 = 7 days) caps maximum mute duration
        *   Optional: `steam.chat.admin.ignore_muted_commands` (default `true`) ignores commands from muted users
    *   **Steam Chat:**
        *   Set `steam.chat.group_id` to your Steam group ID
        *   Set `steam.chat.chat_id` to the default chat channel ID (call-for-sub and general bot traffic)
        *   Optional: `plan_chat_id` + `dedicated_plan_chat` to route `!plan` responses to a planning channel; `plan_chat_keep_clean` removes non-bot messages there; `plan_mention_user` appends the caller's mention
        *   Optional: `poll_chat_id` + `dedicated_poll_chat` to route `!poll` posts to a polls channel; `poll_chat_remove_command_message` deletes the invoking `!poll` line; `poll_mention_user` appends the caller's mention to the question
        *   Set `commands_without_mention = true` to allow `!plan`, `!poll`, etc. without `@`mentioning the bot
    *   **Note:** JSON database files (`cmds.json`, `binds.json`, `bind_sgs.json`) and long-term mute state (`muted_users.json`) are created automatically in the executable directory when needed.
4.  **Set up FastDL (Optional):**
    If you want to use the FastDL server feature:
    *   Create a `fastdl` directory in your project root: `mkdir fastdl`
    *   Place your game content (maps, models, sounds, MOTDs) in the `fastdl` directory
    *   The FastDL server will be available at `http://your-domain/fastdl/`
    *   Directory listings will automatically show for folders without `index.html` files
    *   Files will be served with 1-year cache headers for optimal performance
5.  **Build and Run:**
    ```bash
    cargo build --release
    cargo run --release -- --service
    ```
    *   `--service` starts the RESTful server.
    *   Without `--service`, you can use the CLI to query L4D2 servers.
    *   Example: `cargo run --release -- query -i 51.83.217.86 -p 29800`

    Or deploy the executable: `target/release/Kether_Internal_Services_Server` with the JSON database files, config, and optionally a systemd-user service (put the first three under $HOME/Kether_Internal_Services_Server, systemd service requires `screen` to be installed).
    And run
    ```bash
    Kether_Internal_Services_Server/Kether_Internal_Services_Server -s
    ```

6. **Access the API:**
    * Once the server is running, you can access the API endpoints at `http://localhost:3001/api/...` (or the port you configured).
    * **Database Endpoints:**
        * `/api/binds/*` - Get, add, update, delete binds (with embedded voting)
        * `/api/commands/*` - Get, add, update, delete commands
        * `/api/bind_suggestions/*` - Get, add, update, delete bind suggestions
        * `/api/bind_votings/*` - Vote on binds (add/remove votes; no public voter enumeration)
    * **Other Endpoints:**
        * `/api/LiveServerInfo` - Get live L4D2 server information
        * `/api/steam/*` - Steam user data and game ownership verification
        * `/api/callForSub` - Call for substitute player functionality
        * `/api/ws/plan` - WebSocket stream for lobby reservation updates (`SET <timestamp>` / `CLEAR`)
    * **FastDL Server:** Access your game content at `http://localhost:3001/fastdl/...` (if FastDL feature is enabled).
    * **Directory Listings:** Browse folders at `http://localhost:3001/fastdl/your-folder/` to see automatic directory listings.

## Configuration File (config.toml)

The server uses TOML format for configuration with a clean, nested structure:

```toml
# Kether.pl Server Configuration

[steam]
# Steam Web API key for fetching user data (name, avatar, profile, etc.)
# Get your key from: https://steamcommunity.com/dev/apikey
web_api_key = "YOUR_STEAM_API_KEY"

# Steam Bot Configuration
# These credentials are used for the bot that posts !sub requests to Steam group chat
[steam.bot]
username = "your_steam_username"
password = "your_steam_password"

# Steam Group Chat Configuration
# IDs for the Steam group chat where !sub requests are posted
[steam.chat]
group_id = 103582791429521408
chat_id = 103582791429521409
commands_without_mention = false

# Dedicated planning chat (!plan / !p)
plan_chat_id = 1234567
dedicated_plan_chat = false
plan_chat_keep_clean = false
plan_mention_user = false

# Dedicated poll chat (!poll / !q)
poll_chat_id = 7654321
dedicated_poll_chat = false
poll_chat_remove_command_message = false
poll_mention_user = false

# Admin-only SteamBot commands (!mute / !unmute / !lsmute)
[steam.chat.admin]
admins = []
admins_same_as_frontend = true
mute_max_minutes = 10080
ignore_muted_commands = true

# KetherServerDaemon registry bridge
[server_daemon]
registry_path = "maps_registry.json"
sync_api_key = "shared-secret-with-daemon"
daemon_url = "http://127.0.0.1:8080"
stale_after_secs = 600
```

When website-server and KetherServerDaemon run on separate hosts, set
`daemon_url` to the daemon's reachable HTTP address, configure `sync_api_key`
to match the daemon's `backend_api_key`, and bind the daemon to a reachable interface.
The daemon API is plain HTTP, so firewall its port to the website-server's source
IP only. Prefer a private VPN/tunnel or TLS reverse proxy over an untrusted
network.

**Features:**
- ✅ Auto-generated on first run with helpful comments
- ✅ Type-safe deserialization with serde
- ✅ Nested sections for better organization
- ✅ Clean, modern TOML format
- ✅ Hot reload for most fields when `hot_reload` feature is enabled (read below)

### Config hot reload

With the `hot_reload` feature enabled (default in `kether_meta`), saving `config.toml` triggers an automatic reload. The server logs which fields were applied live and which require a restart.

**Applied immediately (no restart needed):**

* `frontend_admins`
* `steam.web_api_key`
* `server.ip`, `server.port`, `server2.ip`, `server2.port`
* `steam.chat.commands_without_mention`
* `steam.chat.plan_chat_id`, `steam.chat.dedicated_plan_chat`, `steam.chat.plan_chat_keep_clean`, `steam.chat.plan_mention_user`
* `steam.chat.poll_chat_id`, `steam.chat.dedicated_poll_chat`, `steam.chat.poll_chat_remove_command_message`, `steam.chat.poll_mention_user`
* `steam.chat.admin.admins`, `steam.chat.admin.admins_same_as_frontend`, `steam.chat.admin.mute_max_minutes`, `steam.chat.admin.ignore_muted_commands`

**Require a REPL restart (`R` / `restart`) or full process restart:**

* `steam.bot.username`
* `steam.bot.password`
* `steam.chat.group_id`
* `steam.chat.chat_id`

Hot reload updates the shared in-memory config used by REST handlers. SteamBot credentials and chat routing only take effect after a restart, because the bot must log in again and restart its message listener.

## Runtime Management (REPL)

When running with `--service`, the daemon includes a lightweight runtime console.

### REPL console

Press **`C`** (and press Enter) while the service is running to open the REPL. Type **`help`** for a command list.

| Command | Aliases | Action |
|---------|---------|--------|
| `help` | `h` | Show available commands |
| `quit` | `q`, `exit` | Close the REPL (daemon keeps running) |
| `restart` | `R` | Restart REST + SteamBot **in the same process** — stays in the current terminal/`screen` session |
| `stop` | `S` | Shut down the daemon cleanly and exit |
| `chats` | `g`, `groups` | List Steam chat groups and chat rooms with IDs (requires `rest_call_for_sub`) |

After closing the REPL, press **`C`** again to reopen it.

Restart reloads `config.toml` from disk and re-initializes the SteamBot (new login, chat IDs, etc.). Use this after changing bot credentials or chat configuration.

## JSON Database Structure

The server uses three JSON files for data storage:

*   **`cmds.json`** - Server commands in the format:
    ```json
    {
      "1": {
        "command": "!ready",
        "description": "marks yourself Ready"
      }
    }
    ```

*   **`binds.json`** - User binds with embedded voting:
    ```json
    {
      "1": {
        "author": "Player",
        "text": "sorry i am too drunk",
        "upvote": [76561198065000000, 76561198034200001],
        "downvote": []
      }
    }
    ```

*   **`bind_sgs.json`** - Bind suggestions (same structure as binds but without voting arrays).

Files are automatically created on first run if they don't exist. Data is stored in-memory and persisted atomically on each change.

## Feature Flags

The project uses feature flags for modular builds. Default configuration includes all Kether.pl features:

*   **`kether_meta`** (default) - Meta-package including all Kether.pl features
*   **`rest_api`** - Base Rocket REST/WebSocket server (included via `sat` and most API features)
*   **`rest_json_db`** - JSON database REST API (binds, commands, suggestions, voting)
*   **`rest_steam`** - Steam Web API integration
*   **`rest_call_for_sub`** - Call for substitute functionality and SteamBot chat commands
*   **`server_query`** - L4D2 server querying (`!status` and REST `LiveServerInfo`)
*   **`fastdl`** - FastDL file server for Source/GoldSrc games
*   **`hot_reload`** - Event-driven `config.toml` hot reload
*   **`sat`** - Satanixon-specific features

To build with specific features:
```bash
cargo build --release --features "rest_json_db,rest_steam"
```

## Notes

* **Rocket Logging:** To suppress Rocket's "no matching routes" warnings spam in console, set: `export ROCKET_LOG_LEVEL=none` (default is `critical`). Due to how FastDL browser file listing works, as well as random internet bots trying to exploit WordPress api paths (until you have a very effective bot-blocking filters on the Nginx/Apache level), you'll see the server console full of these messages.

* **Migrating from SQLite:** If you have an existing SQLite database, you can export it to the compatible JSON format and place the files in your executable directory. The server will automatically load them on startup.

## License

This project is licensed under the GPL-3.0-only license.
