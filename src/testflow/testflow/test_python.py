import sys
import json


def execute(args):
    return test_python(args['a'])

def test_python(a):
    # aaa
    return 'Hello '+a+' from python'


if __name__ == "__main__":
    import sys
    import json
    print(json.dumps(execute(json.loads(sys.argv[1]))))
