# Security Policy

## Reporting Vulnerabilities

Please report vulnerabilities privately via GitHub Security Advisories:
Go to **Security** > **Advisories** > **Report a vulnerability**.

Do not open public GitHub issues or discussions for security vulnerabilities.

## Secret Management

- Bot tokens (`TELOXIDE_TOKEN`) must only be supplied via environment variables or a local uncommitted `.env` file.
- Never commit credentials, tokens, webhook secrets, or private chat identifiers.
- Pre-commit and CI verification enforce `gitleaks` scans on all commits and working trees.

## Token Leak Incident Response

If a bot token is accidentally exposed:
1. Immediately revoke the token in Telegram via BotFather using `/revoke`.
2. Generate a new bot token with `/token`.
3. Update local environment variables and deployment secrets.
4. Rotate any related webhooks or session data.
