# Lichess Kids

Lichess Kids is a gamified companion application for younger chess players. By winning games against tougher opponents and solving puzzles on Lichess, players earn spins on a weighted wheel, collect coins, purchase accessories, customize their cute cartoon avatars, and see their friends' customized avatars.

---

## ✨ Key Features

*   **Lichess OAuth2 & Mock Login**: Authenticate securely using real Lichess OAuth2 (with PKCE) or bypass it instantly using Developer/Mock Login for local testing.
*   **Aesthetic Wheel Payouts**: Spend earned spins on an animated chess fortune wheel that awards coins matching piece values: Pawn (1c), Knight (3c), Bishop (3c), Rook (5c), or Queen (9c).
*   **Interactive Shop & Avatar Customizer**: Purchase clothing (tops, bottoms, hats, hair, accessories, and backgrounds) and wear them. The avatar preview stacks vector layers dynamically using absolute CSS positioning.
*   **Federated Friends Discovery**:
    *   No manual friends lists! The app automatically syncs the accounts you follow on Lichess who have a Lichess Kids profile.
    *   Discovers remote profiles by searching the followed player's Lichess profile links.
    *   Validates instance software via standard `/.well-known/nodeinfo` and scrapes Schema.org `ProfilePage` structured JSON-LD markup to render remote custom avatars natively.
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
To run the server locally in developer/mock mode:
```bash
cargo run
```
By default, the server listens on **`http://localhost:3000`**. You can open this address in your browser and use the **Mock Login** fields to test all synchronization and shopping mechanics without needing a registered Lichess OAuth client.

To run with real Lichess OAuth authentication:
```bash
LICHESS_CLIENT_ID=your_oauth_client_id \
REDIRECT_URI=http://localhost:3000/api/oauth/callback \
cargo run
```

---

## 🐳 Container Deployment Guide

### 1. Docker Build
To package the app into a slim production-ready image:
```bash
docker build -t lichesskids:latest .
```

### 2. Run Container
Run the container on port `3000` mapping the data directory to persist the SQLite database:
```bash
docker run -d \
  -p 3000:3000 \
  -v lichesskids-data:/data \
  -e DATABASE_URL=/data/lichesskids.db \
  --name lichesskids \
  lichesskids:latest
```

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
