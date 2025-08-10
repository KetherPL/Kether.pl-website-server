# Kether.pl Website Server (Kether Internal Services Server)

This project is the backend server for the Kether.pl website, a homepage for the Polish community-driven [Left 4 Dead 2 ZoneMod-based server](https://github.com/Krevik/Kether.pl-L4D2-Server). It's built using Rust and leverages various libraries for web services, game server querying, and database management.

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
    *   Uses SQLite for persistent data storage.
    *   Manages the following data:
        *   **Binds:** Custom in-game keybinds with author and text.
        *   **Bind Suggestions:** User-submitted bind suggestions.
        *   **Commands:** Server commands with descriptions.
        *   **Bind Votings:** User votes on bind suggestions.
    *   Provides RESTful API endpoints for CRUD (Create, Read, Update, Delete) operations on database entities.
*   **RESTful API:**
    *   Built with the Rocket web framework.
    *   Provides a comprehensive set of API endpoints for interacting with the server's features.
    *   Supports CORS (Cross-Origin Resource Sharing) to allow requests from the Kether.pl frontend (running on `localhost:3000` and `kether.pl`), and the L4D2 server.
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
    *   Uses an `KISS.ini` configuration file to manage settings like:
        *   Database path
        *   Database relative directory flag
        *   Steam Web API key
        *   Steam account credentials for bot functionality
        *   Steam group chat IDs for message delivery
*   **Command-Line Interface (CLI):**
    *   Allows querying L4D2 servers directly from the command line.
    *   Starts the RESTful server service.
* **Error Handling:**
    * Robust error handling for database operations, Steam API calls, and game server queries.
    * Returns appropriate HTTP status codes (e.g., 404 Not Found, 400 Bad Request, 500 Internal Server Error) for API requests.
* **Logging:**
    * Logs database errors to the console for debugging.

## Dependencies

*   **Rocket:** Web framework for building the RESTful API.
*   **gamedig:** Library for querying game servers.
*   **steam-rs:** Library for interacting with the Steam Web API.
*   **SC_Sub_Poster:** Custom library for Steam chat functionality and bot integration.
*   **diesel:** ORM (Object-Relational Mapper) for database interactions.
*   **r2d2:** Connection pooling for the database.
*   **rocket_cors:** CORS support for the Rocket web server.
*   **clap:** Command-line argument parsing.
*   **ini:** Configuration file parsing.
*   **colored:** Terminal output coloring.
*   **tokio:** Async runtime for concurrent operations.
*   **smol:** Async runtime for efficient file system operations in FastDL server.

## Getting Started

1.  **Install Rust:** If you don't have Rust installed, follow the instructions on the official Rust website (https://www.rust-lang.org/tools/install).
2.  **Clone the Repository:**
    ```bash
    git clone <repository_url>
    cd Kether.pl-website-server
    ```
3.  **Create `KISS.ini`:** Create a `KISS.ini` file in the project's root directory (or it will be created automatically on first run).
    *   Set the `database_path` to the desired location and name of your SQLite database file.
    *   Set `database_relative_dir` to `true` if the database is in the same directory as the executable.
    *   Obtain a Steam Web API key from Steam and set it in `steam_web_api_key`.
    *   For Steam Bot functionality, add your Steam account credentials:
        *   Set `steam_account` to your Steam username
        *   Set `steam_password` to your Steam password
    *   For call-for-sub functionality, configure the Steam group chat:
        *   Set `chat_group_id` to your Steam group ID
        *   Set `chat_id` to the specific chat channel ID
4.  **Run Database Migrations:**
    Requires `diesel-cli` to be installed. Either through `cargo install diesel-cli` or your OS's package manager (if supported).
    ```bash
    diesel setup
    diesel migration run
    ```
5.  **Set up FastDL (Optional):**
    If you want to use the FastDL server feature:
    *   Create a `fastdl` directory in your project root: `mkdir fastdl`
    *   Place your game content (maps, models, sounds, MOTDs) in the `fastdl` directory
    *   The FastDL server will be available at `http://your-domain/fastdl/`
    *   Directory listings will automatically show for folders without `index.html` files
    *   Files will be served with 1-year cache headers for optimal performance
6.  **Build and Run:**
    ```bash
    cargo build --release
    cargo run --release -- --service
    ```
    *   `--service` starts the RESTful server.
    *   Without `--service`, you can use the CLI to query L4D2 servers.
    *   Example: `cargo run --release -- query -i 51.83.217.86 -p 29800`

    Or deploy the executable: `target/release/Kether_Internal_Services_Server` with the database, config, and optionally a systemd-user service (put the first three under $HOME/Kether_Internal_Services_Server, systemd service requires `screen` to be installed).
    And run
    ```bash
    Kether_Internal_Services_Server/Kether_Internal_Services_Server -s
    ```

6. **Access the API:**
    * Once the server is running, you can access the API endpoints at `http://localhost:3001/api/...` (or the port you configured).
    * **FastDL Server:** Access your game content at `http://localhost:3001/fastdl/...` (if FastDL feature is enabled).
    * **Directory Listings:** Browse folders at `http://localhost:3001/fastdl/your-folder/` to see automatic directory listings.

## License

This project is licensed under the GPL-3.0-only license.
