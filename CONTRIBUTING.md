# Contributing to PgShield-rs 🛡️

Thank you for your interest in contributing to **PgShield-rs**! We welcome contributions from the community to help make PostgreSQL cloud infrastructure safer, faster, and more resilient.

---

## 📜 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Pull Requests](#pull-requests)
- [Development Setup](#development-setup)
- [Adding Custom Firewall Rules](#adding-custom-firewall-rules)
- [Coding Standards & Guidelines](#coding-standards--guidelines)

---

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## How to Contribute

### Reporting Bugs
Before submitting a bug report, please check existing GitHub issues. If you find a new bug, please open an issue including:
- Your operating system and Rust version (`rustc --version`)
- Reproduction steps and example SQL statement (if relevant)
- Expected vs. actual behavior
- Relevant log output (`RUST_LOG=debug cargo run`)

### Suggesting Enhancements
Feature requests are welcome! Please open an issue detailing:
- The problem your proposal solves
- Proposed solution or API design
- Potential impact on proxy latency or protocol compatibility

### Branching Strategy & Workflow

We follow a Git-flow inspired branching model for stability and automated CI:

- 🚀 **`main`**: Production-ready stable branch. Direct commits are restricted. All releases and tags are cut from `main`.
- 🛠️ **`develop`**: Active integration branch for upcoming features. PRs from feature branches should target `develop`.
- 🌿 **`feature/*` or `fix/*`**: Contributor branches created for specific features or bug fixes.

### Pull Requests
1. Fork the repository and create your feature branch from `develop`:
   ```bash
   git checkout -b feature/my-cool-rule
   ```
2. Ensure your changes compile cleanly and pass all tests:
   ```bash
   cargo check
   cargo test
   ```
3. Format your code according to standard Rust style:
   ```bash
   cargo fmt
   cargo clippy
   ```
4. Push to your fork and submit a Pull Request to `main`.

---

## Development Setup

Prerequisites:
- **Rust toolchain** (v1.75+): Install via [rustup](https://rustup.rs/)
- **Docker & Docker Compose**: For local PostgreSQL integration testing

Quick start:
```bash
# Clone the repository
git clone https://github.com/aeroforge-labs/PgShield-rs.git
cd PgShield-rs

# Run tests
cargo test

# Run the proxy locally
cargo run -- --listen-addr 127.0.0.1:6432 --backend-host 127.0.0.1 --backend-port 5432
```

---

## Adding Custom Firewall Rules

PgShield-rs makes it easy to add custom AST firewall inspection rules:

1. Create a new struct in `src/firewall/rules/mod.rs` implementing `FirewallRule`:
   ```rust
   pub struct MyCustomRule;

   impl FirewallRule for MyCustomRule {
       fn name(&self) -> &'static str {
           "MyCustomRule"
       }

       fn evaluate(&self, statement: &Statement) -> Result<(), RuleViolation> {
           // Inspect statement AST...
           Ok(())
       }
   }
   ```
2. Register the rule in `QueryFirewall::new()` inside `src/firewall/engine.rs`.
3. Add a unit test in `tests/firewall_test.rs`.

---

## Security

If you discover a security vulnerability, please refer to our [Security Policy](SECURITY.md) for disclosure details instead of opening a public issue.
