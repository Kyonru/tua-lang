# Security Policy

Tua is a compiler, CLI, language server, and editor extension that processes
project-controlled input. Security reports are welcome, especially for
filesystem escape, symlink traversal, unintended command execution, generated
code injection, protocol or media denial of service, dependency compromise, or
release credential exposure.

## Supported Versions

Security fixes are made for the latest published release and the current
`main` branch. Earlier releases are not maintained separately; upgrade to the
latest release before reporting an issue that may already be fixed.

| Version | Security updates |
| --- | --- |
| Latest release | Yes |
| Current `main` | Yes |
| Earlier releases | No |

## Report A Vulnerability Privately

Use GitHub's **Report a vulnerability** form in this repository's Security tab:

<https://github.com/Kyonru/tua-lang/security/advisories/new>

If GitHub does not make private reporting available to your account, open a
public issue containing only a request for a private maintainer contact. Do not
include the vulnerability, exploit, affected paths, or proof of concept in that
issue.

Include as much of the following as is safe:

- the affected Tua version or commit and installation method;
- the affected component and operating system;
- attacker capabilities and the security property that fails;
- minimal reproduction steps or a proof of concept;
- impact, including files, commands, data, or credentials at risk;
- whether the issue is already public or known to be exploited;
- a suggested remediation or regression test, if available.

Remove real credentials and private project data. Small synthetic fixtures are
preferred. Please allow maintainers time to reproduce, fix, test, and publish a
release before public disclosure. This project is maintained on a best-effort
basis and does not promise a response or remediation SLA.

## Security Model

Tua's parser, checker, formatter, linter, language server, read-only API, and
MCP adapter analyze source without executing Tua or Lua code. Project source,
`tua.toml`, Lua files, assets, symlinks, LSP messages, and API/MCP messages must
still be treated as untrusted data.

The following boundaries are intentional:

- CLI build output must remain inside the project root. Output paths that
  escape it, or writes through symlink components, are rejected.
- LSP and newline-delimited API/MCP transports enforce message limits, and the
  language server limits queued and in-flight work. These limits reduce denial
  of service risk; they are not a substitute for operating-system isolation.
- Built-in audio analysis applies format, duration, channel, decoded-packet,
  and analysis-memory limits.
- The VS Code extension has limited support in Restricted Mode. It may start
  only its bundled Tua server. Custom server executables, LOVE execution,
  debugging, and project initialization require workspace trust.
- External asset importers are opt-in commands from project configuration.
  Tua gives them bounded time and diagnostic output and validates their
  manifests and destinations, but it does **not** sandbox the importer process.
  Running `tua check`, `build`, or `watch` on such a project grants that command
  the invoking user's operating-system permissions.
- A configured custom Tua executable and LOVE executable are trusted local
  tools. Workspace trust does not authenticate those binaries.
- The optional Ollama completion feature talks to the locally configured
  Ollama endpoint only when enabled. Review the project and editor settings
  before enabling model-assisted completion.

For hostile repositories, inspect `tua.toml` before invoking the CLI, keep VS
Code in Restricted Mode, and use an operating-system sandbox or disposable
environment when stronger containment is required.

## Release And Dependency Security

Open or untrusted pull requests do not run repository code in GitHub Actions.
Do not propose `pull_request_target` as a substitute. Trusted pushes, manual
dispatches, and releases run the repository's verification workflows.

Release workflows use least-privilege job permissions. A version tag must
match repository metadata, trusted verification must pass before packaging,
and published artifacts are checked against the release checksum manifest.
Marketplace credentials are exposed only to their publishing step, which uses
tools installed from the committed npm lockfile.

Published binaries and VSIX files are checksummed but are not currently
code-signed. Verify downloads as described in
[docs/releases.md](docs/releases.md#verify-a-download).

Routine dependency reports that do not demonstrate a vulnerability in Tua may
use the bug template. Reports involving a reachable exploit, compromised
package, malicious release asset, or credential exposure should use private
reporting.
