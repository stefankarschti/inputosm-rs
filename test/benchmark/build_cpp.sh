#!/bin/sh
set -eu
cpp_source=${1:-"$HOME/inputosm"}
cpp_build=${2:-"$cpp_source/build"}
output=${3:-target/comparison/cpp}
mkdir -p "$(dirname "$output")"
clang++-19 -std=c++20 -O3 -DNDEBUG \
  -I"$cpp_source/include" \
  -I"$cpp_build/_deps/fmt-src/include" \
  -I"$cpp_build/_deps/libdeflate-src" \
  "$cpp_source/src/inputosm.cpp" \
  "$cpp_source/src/inputosmpbf.cpp" \
  "$cpp_source/src/inputosmxml.cpp" \
  "$cpp_source/src/timeutil.cpp" \
  "$cpp_source/src/inputosmlog.cpp" \
  "$cpp_source/test/benchmark/pbf_benchmark.cpp" \
  "$cpp_build/_deps/libdeflate-build/libdeflate.a" \
  "$cpp_build/_deps/fmt-build/libfmt.a" \
  -lexpat -pthread -o "$output"
