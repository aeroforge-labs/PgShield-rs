# Security Policy 🛡️

The security of **PgShield-rs** and the databases it protects is our top priority. We appreciate the work of security researchers and developers in keeping the open-source ecosystem safe.

---

## Supported Versions

Only the latest release version on `main` receives active security updates and patches:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1.0 | :x:                |

---

## Reporting a Vulnerability

**Please DO NOT open a public GitHub issue for security vulnerabilities.**

If you discover a security vulnerability (such as a wire protocol bypass, SQL AST inspection bypass, or denial-of-service vector), please follow these steps:

1. Send an email to **security@aeroforge.io** (or contact maintainers directly via GitHub private vulnerability reporting).
2. Include a detailed description of the issue:
   - Type of vulnerability (e.g. Protocol Injection, AST Parser Bypass, Memory Leak).
   - Step-by-step reproduction instructions or proof-of-concept SQL statement.
   - Potential impact on proxy latency or backend PostgreSQL server.

---

## Response Process

- **Acknowledgment:** We will acknowledge receipt of your vulnerability report within **48 hours**.
- **Assessment:** Maintainers will evaluate the issue and confirm severity within **5 business days**.
- **Fix & Disclosure:** A patch will be prepared, tested, and released as a minor/patch version. Coordinated public disclosure will follow after users have been given adequate time to upgrade.
