# Rust Media Dashboard

A lightweight, modern media management dashboard built with Rust. This application provides a unified interface to monitor and manage your media services (Sonarr, Radarr, Jackett, and Transmission) with a focus on ease of use and visual clarity.

![Media Dashboard Preview](static/screenshot_preview.png) *(Placeholder for screenshot - use the dashboard to generate your own!)*

## 🚀 Features

### 📊 Service Monitoring
- **Real-time Status**: Live heartbeats for Sonarr, Radarr, Jackett, and Transmission.
- **Enhanced Library Stats**:
  - **Sonarr**: Total series and missing episodes count.
  - **Radarr**: Total movies and missing/un-downloaded count.
  - **Transmission**: Active download count with live progress/names.
  - **Jackett**: Health status of configured indexers with offline tracker alerts.

### 🎬 Media Management (CRUD)
- **TV Shows (Sonarr)**:
  - Search TVDB for new series.
  - Add series with root folder and quality profile selection.
  - List and remove series.
- **Movies (Radarr)**:
  - Search TMDB for movies.
  - Add movies with full configuration.
  - List and remove movies.
- **Torrents (Transmission)**:
  - List active torrents with progress bars, speeds, and ETA.
  - Add torrents via magnet links or .torrent URLs.
  - Remove torrents with an option to delete downloaded data.
- **Indexers (Jackett)**:
  - View all configured indexers and their health.
  - Quick link to Jackett Web UI for management.

### 🛠 System Features
- **Audit Logs**: Track dashboard actions (add/remove) across all services.
- **Centralized Configuration**: Simple UI to manage service URLs, API keys, and credentials.
- **Material Dark Theme**: Sleek, responsive interface built with modern CSS and Material Design icons.

## 🛠 Tech Stack

- **Backend**: [Rust](https://www.rust-lang.org/) with [Axum](https://github.com/tokio-rs/axum) (Web Framework)
- **Database**: [SQLite](https://sqlite.org/) via [SQLx](https://github.com/launchbadge/sqlx) for configuration and logs
- **HTTP Client**: [reqwest](https://github.com/seanmonstar/reqwest)
- **Frontend**: Vanilla HTML5, CSS3, and JavaScript (No heavy frameworks for maximum performance)
- **Styling**: Material Design (Roboto Typography, Material Icons)

## 📦 Installation & Setup

The easiest way to run the dashboard is using **Docker Compose**.

### Prereqs
- Docker
- Docker Compose

### Quick Start

1. Clone the repository:
   ```bash
   git clone https://github.com/aragorn2909/media-dashboard.git
   cd media-dashboard
   ```

2. Build and run the container:
   ```bash
   docker compose up -d --build
   ```

3. Access the dashboard:
   Open your browser and go to `http://localhost:7778`.

4. Configure your services:
   Navigate to the **Settings** page and enter your URLs and API keys for Sonarr, Radarr, Jackett, and Transmission.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 🛡️ Security & Disclaimer

**This project was primarily "vibe-coded" using AI assistance.** As an author actively learning about software development and security practices, I cannot guarantee the absolute safety or stability of this code. 

**Please use this software at your own risk.**

**Security Feedback:** I welcome and greatly appreciate any security audits, feedback, or vulnerability disclosures from the community. If you find a flaw, please open an issue or submit a PR so I can learn and improve the project!

## 📜 License

GPL 2.0 License - see [LICENSE](LICENSE) for details.
