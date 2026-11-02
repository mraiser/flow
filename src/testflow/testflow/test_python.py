import sys
import json


from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from collections.abc import Mapping, Sequence
    from typing import Any
    from ndata_py import NDataArray, NDataBytes, NDataObject

def execute(args):
    return test_python(args['a'])

def test_python(a: "str") -> "str":
    # aaa
    return 'Hello '+a+' from python'


if __name__ == "__main__":
    import sys
    import json
    print(json.dumps(execute(json.loads(sys.argv[1]))))
