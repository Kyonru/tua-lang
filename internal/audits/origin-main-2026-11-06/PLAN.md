# Origin/main change audit and remediation

Baseline comparison: b40dbb445f7d8ba36e9312909be7c5d2949ac4af → d7aed2e5f1297b80a948e055985375bceaff6f0c (203 files). Uses local origin/main. Prior developer-experience audit is preserved as baseline evidence; its findings are not automatically attributed to this change set.

| Lane | Scope | Owner |
|---|---|---|
| Language contracts | Checker, inference, narrowing, scope, Lua interoperability | typing agent |
| Editor semantics | Completion, hover, signature help, recovery, class receivers | completion agent |
| Lua consistency | Parser, lowering, emission, source maps, class/runtime reload behavior | emission agent |
| Protocol/project | Encoding, request routing, workspace ownership, scheduling/cache behavior | lead |
| Toolchain and safety | CLI testing/build output, extension integration, trust boundaries | follow-up agent + lead |
| Documentation and verification | Changed contract claims, roadmap, examples, CI gates, integrated regressions | lead + follow-up review |

Audit before changing each behavior. Confirm with focused repros; implement shared fixes with regressions. Preserve existing artifacts. Defer language-design decisions and environment-dependent qualification rather than invent support. No publication or external messaging. Final cross-crate gate after integration. Reports distinguish previously existing defects, changed behavior and unproven regression attribution.
