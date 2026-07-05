# Lichess Kids

Lichess Kids is a gamified companion application for younger chess players. By winning games against tougher opponents and solving puzzles on Lichess, players earn spins on a weighted wheel, collect coins, purchase accessories, customize their cute cartoon avatars, and see their friends' customized avatars.

---

## ✨ Key Features

*   **Lichess OAuth2 Login**: Authenticate securely using real Lichess OAuth2 (with PKCE).
*   **Aesthetic Wheel Payouts**: Spend earned spins on an animated chess fortune wheel that awards coins matching piece values: Pawn (1c), Knight (3c), Bishop (3c), Rook (5c), or Queen (9c).
*   **Interactive Shop & Avatar Customizer**: Purchase clothing (tops, bottoms, hats, hair, accessories, and backgrounds) and wear them. The avatar preview stacks vector layers dynamically using absolute CSS positioning.
*   **Local Friends Discovery**: Sync followed users automatically from Lichess. If a followed user has a local Lichess Kids profile registered on this instance, they will automatically appear in your friends list with their custom equipped avatar.
*   **Robust Backend**: Written in Rust using the Axum framework and SQLite (`rusqlite`) database with concurrent transaction locks.
*   **Modern DevOps**: Includes multi-stage Docker builds, rootless Podman Quadlet configuration, and a GitHub Actions CI test pipeline.

---

## 🛠️ Build and Development Guide

### 1. Prerequisites
Ensure you have the following system dependencies installed (on Ubuntu/Debian):
```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev sqlite3
```

Ensure the Rust toolchain is installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Compile and Test
Compile the debug binary:
```bash
cargo build
```

Run the database integration tests:
```bash
cargo test
```

### 3. Run Locally
To run the server locally:
```bash
cargo run
```
By default, the server listens on **`http://localhost:64355`**. You can open this address in your browser to log in using Lichess OAuth2 PKCE.

> [!NOTE]
> Lichess Kids utilizes Lichess's public OAuth2 with PKCE (Proof Key for Code Exchange) flow. **You do not need to register a client secret, specify a redirect URI environment variable, or request a Lichess API key.** The redirect URI is automatically constructed dynamically from the request headers sent by the client browser or reverse proxy, making deployment simpler and zero-config.

---

## 🐳 Container Deployment Guide

### 1. Docker Build
To package the app into a slim production-ready image:
```bash
docker build -t lichesskids:latest .
```

### 2. Run Container
Run the container on port `64355` mapping the data directory (to persist the SQLite database) and the assets configuration volume (to override custom characters, shop accessories, and spin rules):
```bash
docker run -d \
  -p 64355:64355 \
  -v lichesskids-data:/data \
  -v /path/to/custom-assets:/app/assets \
  -e DATABASE_URL=/data/lichesskids.db \
  --name lichesskids \
  lichesskids:latest
```

### 3. Customizing Avatars, Accessories, and Spin Rules

By mounting a host directory to `/app/assets`, you can fully customize the content and rules of your instance:

#### A. Adding or Removing Custom Avatar Bases
1. **Add Base**: Drop a valid standalone SVG file in `/app/assets/bases/[id].svg` (e.g., `viewBox="0 0 200 200"`), and append a record to the `"bases"` list in `/app/assets/metadata.json`:
   ```json
   {
     "id": "rabbit",
     "name": "Clever Rabbit"
   }
   ```
2. **Remove Base**: Delete the `.svg` file from the `bases` directory and remove its entry from `metadata.json`.
3. **Restart**: Restart the container to reload the catalog dynamically.

#### B. Adding or Removing Accessories
1. **Add Item**: Place the accessory SVG file in `/app/assets/items/[id].svg` and add it to the `"items"` array in `metadata.json`:
   ```json
   {
     "id": "wizard_hat",
     "name": "Wizard Hat",
     "category": "hat",
     "price": 20
   }
   ```
   *Note: Supported categories are `hat`, `hair`, `top`, `bottom`, `accessory`, and `background`.*
2. **Remove Item**: Delete the `.svg` file from the `items` directory and remove its entry from `metadata.json`.

#### C. Customizing Spin Rules
The sync performance requirements can be configured in `/app/assets/metadata.json` under `"spin_rules"`:
```json
"spin_rules": {
  "game_rating_offset": -100,
  "puzzle_rating_offset": -100,
  "puzzles_per_spin": 25
}
```
*   `game_rating_offset` / `puzzle_rating_offset`: Ratings must be within this offset (or higher) relative to the player's rating at the time of the play. Offset `-100` allows players to win spins for matches and puzzles up to 100 points below their current rating.
*   `puzzles_per_spin`: The number of tactical puzzle solutions required to earn a spin.

#### ⚠️ Why Daily Spins are Forbidden
Spins cannot be awarded for daily logins or daily check-ins. Lichess Kids is designed to encourage learning and practice. Spins and rewards must only be earned as an accomplishment for solving puzzles or winning matches against challenging opponents. Leaving daily spins disabled prevents gamification loop exploitation and focuses the reward feedback loop strictly on active effort.

### 4. Reverse Proxy & SSL Configuration (Recommended)

To ensure secure connection delivery (HTTPS), **it is highly recommended to run this server behind a reverse proxy (such as Nginx, Traefik, or Caddy) or a Cloudflare tunnel.**

We provide copy-pasteable configuration templates in the `deploy/reverse-proxy/` directory:
*   **[Nginx (Debian sites-available style)](file:///home/devesh/projects/lichesskids/deploy/reverse-proxy/nginx.conf)**: Configures ports `80`/`443`, handles the ACME Let's Encrypt directory verification challenges, redirects traffic to HTTPS, and maps headers.
*   **[Traefik Dynamic YAML](file:///home/devesh/projects/lichesskids/deploy/reverse-proxy/traefik.yaml)**: Configures HTTP-to-HTTPS redirect middleware and routing rules mapping SSL certificates.
*   **[Caddyfile](file:///home/devesh/projects/lichesskids/deploy/reverse-proxy/Caddyfile)**: A minimal 5-line configuration that automatically provisions SSL and forwards requests.

#### Cloudflare Tunnel Alternative
If you prefer not to open ports or manage SSL certificates locally, you can route container traffic through a **Cloudflare Tunnel (`cloudflared`)**. Create a tunnel mapping your domain to `http://localhost:64355`. Ensure that Cloudflare SSL is set to **Full** or **Full (Strict)**, and enable HTTP Header forwarding so the server receives correct host information.

---

## ⚙️ Podman Quadlet Rootless Deployment

We provide standard Quadlet unit files under `deploy/quadlet/` to run Lichess Kids as a rootless systemd user service.

1.  Copy the Quadlet configuration files to your systemd user directory:
    ```bash
    mkdir -p ~/.config/containers/systemd/
    cp deploy/quadlet/* ~/.config/containers/systemd/
    ```

2.  Reload the user systemd daemon to generate the units:
    ```bash
    systemctl --user daemon-reload
    ```

3.  Start and enable the container service:
    ```bash
    systemctl --user start lichesskids.service
    ```
    To automatically start the container on user login, enable it:
    ```bash
    systemctl --user enable lichesskids.service
    ```

---

## 📄 License

This project is licensed under the terms of the **GNU GPLv3** license. Refer to the [LICENSE](LICENSE) file for the full text.
