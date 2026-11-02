# Type stubs for flowlang, the runtime module the flowlang embedded Python
# runtime provides (in subprocess mode, call() raises RuntimeError). flowb
# writes this file - this base plus an `api` section generated from the
# libraries in data/ - next to generated Python commands.
from collections.abc import Mapping, Sequence
from typing import Any

from ndata_py import NDataArray, NDataBytes, NDataObject

class CommandError(Exception):
    """A command run by call() failed: it raised or panicked, or returned
    {"status": "err", ...}. str(e) is its message."""

def call(lib: str, ctl: str, cmd: str, /, *args: Any, **kwargs: Any) -> Any:
    """Runs any command - Rust, Python, flow, JavaScript, Java - and returns
    its value (a flow's single output, or all of its outputs as an
    NDataObject). Positional arguments map onto the declared parameters in
    order; arguments are cast to the declared types. Names resolve exactly,
    or with `-` read as `_`.

    Raises LookupError for an unknown command, TypeError for arguments that
    do not fit the declared parameters, CommandError when the command fails."""
