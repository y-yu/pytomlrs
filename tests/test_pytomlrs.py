import pytomlrs

python_value = {
    'a': {
        'b': 1,
        'c': True
    },
    'd': {
        'e': [1.2, 'foobar']
    }
}
expected_toml = """
[a]
b = 1
c = true

[d]
e = [1.2, "foobar"]
""".strip()

def test_to_toml():
    actual = pytomlrs.to_toml(python_value)
    assert actual.strip() == expected_toml

def test_from_toml():
    actual = pytomlrs.from_toml(expected_toml)
    assert actual == python_value
