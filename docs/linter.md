---
icon: lucide/list-checks
---

# Linter

The linter owns code-quality guidance. Its role is comparable to ESLint: it
reports suspicious constructs without rewriting presentation. `tua lint`
enables Tua's standard rules automatically and reports syntax and lint
diagnostics; use `tua check` for type, project, and domain diagnostics.

```bash
tua lint
tua lint --strict
```

`--strict` adds every Tua-specific optional lint to the standard rules. The
standard rules are also enabled by default in `tua check` and the editor. A
project can opt out of that default without changing `tua lint`:

```toml title="tua.toml"
[diagnostics]
rules = false
```

## Rules

Tua's standard rules cover portable Lua/Tua code-quality checks. Several rules
share an existing Tua code because they are the same analysis.

| Rule | Tua diagnostic |
| --- | --- |
| `UnknownGlobal` | `TL4004` |
| `GlobalUsedAsLocal` | `TL4052` |
| `LocalShadow` | `TL4035` |
| `SameLineStatement` | `TL4036` |
| `MultiLineStatement` | `TL4037` |
| `LocalUnused`, `FunctionUnused` | `TL4005` |
| `ImportUnused` | `TL4054` |
| `BuiltinGlobalWrite` | `TL4038` |
| `PlaceholderRead` | `TL4039` |
| `UnreachableCode` | `TL4040` |
| `UnknownType` | `TL4041` |
| `ForRange` | `TL4042` |
| `UnbalancedAssignment` | `TL4027`, `TL4028` |
| `ImplicitReturn` | `TL4053` |
| `DuplicateLocal` | `TL4043` |
| `FormatString` | `TL4044` |
| `TableLiteral` | `TL4001` |
| `UninitializedLocal` | `TL4045` |
| `DuplicateFunction` | `TL4046` |
| `TableOperations` | `TL4047` |
| `DuplicateCondition` | `TL4012`, `TL4013` |
| `MisleadingAndOr` | `TL4048` |
| `CommentDirective` | `TL4049` |
| `IntegerParsing` | `TL4050` |
| `ComparisonPrecedence` | `TL4051` |

`DeprecatedGlobal` depends on a host-provided deprecated-global catalog, and
`DeprecatedApi` depends on deprecation metadata for the resolved external API.
Tua does not invent either catalog, so those two rules are not emitted yet.
Likewise, dialect-specific table APIs are ignored on Tua's LuaJIT target. Tua
does report unsupported `--!` directives so foreign configuration comments
never silently appear to affect Tua.

The repository's `examples/linter` fixture contains an intentionally broken
case for every rule Tua can currently emit in the standard set. Running
`tua lint` there is expected to exit with status `1`.

## Formatter Boundary

`SameLineStatement` and `MultiLineStatement` remain lints because they identify
ambiguous statement structure. Whitespace, wrapping, quote style, indentation
width, and empty-line normalization belong only to the [Formatter](formatter.md).
