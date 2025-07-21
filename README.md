A fast, modern HTTP server written in Rust, inspired by Nginx. Supports static files, directory listing, custom error pages, file uploads, CGI (Python), redirects, and method-based access control.

## 🚀 Features

- **Static File Serving** 📄
  - Configurable root directory
- **Directory Listing** 📁


# localhost - Rust HTTP Server

A fast, modern HTTP server written in Rust, inspired by Nginx. Supports static files, directory listing, custom error pages, file uploads, CGI (Python), redirects, and method-based access control.

## ✨ Features

- 🌐 **Serve Static Files**: Fast and secure file serving from configurable directories.
- 📁 **Directory Listing**: Autoindex for browsing folders.
- 📤 **File Uploads**: Multipart/form-data upload support.
- 🐍 **CGI Support**: Run Python scripts for dynamic content.
- 🔄 **Redirects**: HTTP 301/302 redirection.
- 🔒 **Method-Based Access Control**: Restrict HTTP methods per route.
- ⚠️ **Custom Error Pages**: Serve your own 404, 403, 500, etc.
- 📝 **Configurable**: Nginx-style config file for routes, roots, methods, uploads, CGI, error pages.

## 🛠️ Technology Stack

<img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white">
<img alt="Linux" src="https://img.shields.io/badge/Linux-333333?style=for-the-badge&logo=linux&logoColor=white">
<img alt="Python" src="https://img.shields.io/badge/Python-3776AB?style=for-the-badge&logo=python&logoColor=white">

## 🏗 Architecture

- **Static Handler:** `static_handler.rs`
- **CGI Handler:** `cgi`
- **Config Parser:** `config`
- **HTTP Core:** `http`
- **Server Engine:** `server`

## 🚦 Quick Start

### Prerequisites
- Rust (via [rustup](https://rustup.rs/))
- Python 3 (for CGI)
- Linux (recommended)


### Installation

```bash
git clone https://learn.reboot01.com/git/aaljuffa/localhost.git
cd localhost
cargo build --release
cargo run --release
```

The server will be available at:
- Main page: http://localhost:8080/
- API, uploads, CGI, etc. as per your config

## � Configuration Example

See [`config/webserv.conf`](config/webserv.conf):

```nginx
server {
    listen 8080;
    root ./www;
    error_page 404 /404.html;

## 👥 Authors

- Amir Iqbal - [@chefaiqbal](https://github.com/chefaiqbal)
- Abdulla Aljuffairi - [xoabdulla](https://learn.reboot01.com/xoabdulla)

Enjoy your fast, secure, and extensible Rust HTTP server! �
│   ├── static_handler.rs
│   ├── cgi/
│   ├── config/
│   ├── http/
│   ├── server/
│   └── utils/
├── www/
│   ├── index.html
│   ├── uploads/
│   ├── cgi-bin/
│   └── error pages
├── Cargo.toml
└── README.md
```

## ❓ FAQ

- **How do I change the port?**
  - Edit `listen` in `config/webserv.conf`.
- **How do I add a new route?**
  - Add a new `location` block in the config.
- **How do I enable uploads?**
  - Set `upload_store` in a location block.
- **How do I run CGI scripts?**
  - Place Python scripts in `cgi-bin` and set `cgi_pass python`.
- **How do I see logs?**
  - Logs are printed to the console. Run with `RUST_LOG=debug cargo run --release` for verbose output.

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 📄 License

MIT License - see the LICENSE file for details.

## 👥 Authors

- Amir Iqbal - [@chefaiqbal](https://github.com/chefaiqbal)

Enjoy your fast, secure, and extensible Rust HTTP server! 🚀
- **Memory-safe**: Pure Rust

---

## 🧑‍� Development

- Edit Rust source in [`src/`](src/)
- Add CGI scripts in [`www/cgi-bin/`](www/cgi-bin/)
- Add static files in [`www/`](www/)

---

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## 📄 License

MIT License - see the LICENSE file for details.

---

## 👤 Author

- Amir Iqbal - [@chefaiqbal](https://github.com/chefaiqbal)

---

Enjoy your fast, modern Rust HTTP server! 🚀
```

Or run the built binary:

```bash
