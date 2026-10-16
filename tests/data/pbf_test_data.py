"""Build small PBF fixtures with Python standard-library modules."""
import struct
import zlib


def var(value):
    result = bytearray()
    while value >= 128:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return bytes(result)


def integer(number, value):
    return var(number << 3) + var(value & ((1 << 64) - 1))


def message(number, value):
    return var((number << 3) | 2) + var(len(value)) + value


def sint(value):
    return (value << 1) ^ (value >> 63)


def packed(number, values):
    return message(number, b"".join(var(v) for v in values))


def block(kind, raw, compressed=False):
    blob = integer(2, len(raw)) + message(3, zlib.compress(raw)) if compressed else message(1, raw)
    descriptor = message(1, kind) + integer(3, len(blob))
    return struct.pack(">I", len(descriptor)) + descriptor + blob


def header():
    return block(b"OSMHeader", message(4, b"OsmSchema-V0.6"))


def node(identifier):
    return integer(1, sint(identifier)) + integer(8, 0) + integer(9, 0)


def statistics_fixture():
    one_tag = packed(2, [1]) + packed(3, [2])
    two_tags = packed(2, [1, 3]) + packed(3, [2, 4])
    strings = message(1, b"".join(message(1, s) for s in (b"", b"name", b"value", b"source", b"test")))
    def timestamp(value):
        return message(4, integer(2, value))
    def way(identifier, refs, tags):
        return integer(1, identifier) + packed(8, refs) + tags
    def relation(identifier, count, tags):
        return integer(1, identifier) + tags + (integer(8, 0) + integer(9, sint(1)) + integer(10, 0)) * count
    def group(kind, raw):
        return message(2, message(kind, raw))
    ordinary = message(1, node(10) + two_tags + timestamp(1000000000)) + message(1, node(11))
    dense = (packed(1, [sint(20), sint(1), sint(1)]) + packed(8, [0, 0, 0]) + packed(9, [0, 0, 0]) +
             packed(10, [1, 2, 0, 0, 0]) + message(5, packed(2, [sint(4000000000), sint(-1), 0])))
    first = (strings + integer(18, 2000) + message(2, ordinary) + group(2, dense) +
             group(3, way(30, [2, 2], one_tag) + timestamp(2200000000)) + group(3, way(31, [2, 2, 2], b"")) +
             group(4, relation(40, 2, one_tag)) + group(4, relation(41, 1, two_tags)))
    second = (strings + integer(18, 500) + message(2, message(1, node(99) + one_tag + timestamp(6000000000)) + message(1, node(100))) +
              group(3, way(90, [2], two_tags) + timestamp(7000000000)) + group(4, relation(91, 4, b"") + timestamp(9400000001)))
    return header() + block(b"OSMData", first) + block(b"OSMData", second, True) + block(b"Unused", b"") + block(b"OSMData", strings + message(2, b""))
