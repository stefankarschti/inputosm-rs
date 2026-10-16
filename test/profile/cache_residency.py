#!/usr/bin/env python3
"""Report Linux page-cache residency without reading mapped file contents."""
import argparse
import ctypes
import json
import mmap
import os


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("path")
    args = parser.parse_args()
    libc = ctypes.CDLL(None, use_errno=True)
    libc.mincore.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_ubyte)]
    libc.mincore.restype = ctypes.c_int
    with open(args.path, "rb") as file:
        size = os.fstat(file.fileno()).st_size
        if size == 0:
            parser.error("Input file is empty")
        pages = (size + mmap.PAGESIZE - 1) // mmap.PAGESIZE
        vector = (ctypes.c_ubyte * pages)()
        # ACCESS_COPY permits a buffer address without changing the source file.
        with mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_COPY) as mapping:
            address = ctypes.c_char.from_buffer(mapping)
            result = libc.mincore(ctypes.addressof(address), size, vector)
            del address
            if result != 0:
                code = ctypes.get_errno()
                raise OSError(code, os.strerror(code))
        resident = bytes(vector).translate(bytes(i & 1 for i in range(256))).count(1)
        print(json.dumps({"path": os.path.abspath(args.path), "file_bytes": size,
                          "page_bytes": mmap.PAGESIZE, "pages": pages,
                          "resident_pages": resident, "resident_fraction": resident / pages}))


if __name__ == "__main__":
    main()
