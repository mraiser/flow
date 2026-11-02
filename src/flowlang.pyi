# Type stubs for flowlang, the runtime module the flowlang embedded Python
# runtime provides (in subprocess mode, call() raises RuntimeError). flowb
# writes this file next to generated Python commands.
from typing import Any

class CommandError(Exception):
    """A command run by call() failed: it raised or panicked, or returned
    {"status": "err", ...}. str(e) is its message."""

def call(lib: str, ctl: str, cmd: str, /, **kwargs: Any) -> Any:
    """Runs any command - Rust, Python, flow, JavaScript, Java - with its
    arguments cast to the declared parameter types, and returns its value (a
    flow's single output, or all of its outputs as an NDataObject).

    Raises LookupError for an unknown command, TypeError for an argument that
    cannot be converted or cast, CommandError when the command fails."""
