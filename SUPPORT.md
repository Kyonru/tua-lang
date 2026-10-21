# Tua Support

The documentation site is the best starting point for installation, language,
configuration, CLI, editor, LOVE, asset, and troubleshooting questions:

<https://kyonru.github.io/tua-lang/>

Useful references include the [installation guide](docs/installation.md),
[CLI reference](docs/cli.md), [language guide](docs/language.md),
[language-server guide](docs/lsp.md), and [examples guide](docs/examples.md).

## Ask For Help

Use the GitHub **Support question** issue template when the documentation does
not answer the question. For a reproducible defect, use **Bug report** instead;
for a product proposal, use **Feature request**.

Before filing, collect:

- `tua --version` or the source commit;
- operating system and architecture;
- installation method and editor/extension version, when relevant;
- the exact command and complete diagnostic text;
- a small `.tua` example and sanitized `tua.toml` when possible;
- what you expected, what happened, and what you already tried.

In VS Code, run **Tua: Copy Debug Info** and include the sanitized result. Check
the Tua output channel for server startup or request errors. Remove usernames,
private paths, source code you cannot share, tokens, and other credentials.

Maintainers provide support on a best-effort basis. There is no guaranteed
response time, compatibility guarantee for unreleased forks, or private
debugging service for application-specific game code. A minimal public
reproduction is usually the fastest path to an answer.

## Security And Sensitive Reports

Do not use a public support or bug issue for a suspected vulnerability. Follow
[SECURITY.md](SECURITY.md) and omit all vulnerability details from public
channels. Do not attach proprietary projects or logs containing credentials.

## Contributing A Fix

If you would like to work on the problem, see
[CONTRIBUTING.md](CONTRIBUTING.md). Open pull requests do not run repository
code in GitHub Actions, so contributors must report their local verification
results.
