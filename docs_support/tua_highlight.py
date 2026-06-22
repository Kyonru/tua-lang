"""Register Tua fenced code blocks with Pygments."""

from markdown.extensions import Extension
from pygments.lexers import LEXERS, _lexer_cache

try:
    from pygments.lexers.scripting import LuauLexer as BaseTuaLexer
except ImportError:  # pragma: no cover - compatibility with older Pygments.
    from pygments.lexers.scripting import LuaLexer as BaseTuaLexer


class TuaLexer(BaseTuaLexer):
    """A close-enough lexer for Tua while the language shares Luau syntax."""

    name = "Tua"
    aliases = ["tua"]
    filenames = ["*.tua"]
    mimetypes = ["text/x-tua"]


__all__ = ["TuaLexer"]


def register_tua_lexer() -> None:
    LEXERS["TuaLexer"] = (
        "docs_support.tua_highlight",
        TuaLexer.name,
        tuple(TuaLexer.aliases),
        tuple(TuaLexer.filenames),
        tuple(TuaLexer.mimetypes),
    )
    _lexer_cache[TuaLexer.name] = TuaLexer


class TuaHighlightExtension(Extension):
    def extendMarkdown(self, md) -> None:  # noqa: N802
        register_tua_lexer()


def makeExtension(**kwargs):  # noqa: N802
    return TuaHighlightExtension(**kwargs)


register_tua_lexer()
