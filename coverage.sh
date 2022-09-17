#!/usr/bin/env bash

export RUSTFLAGS='-Cinstrument-coverage'
export LLVM_PROFILE_FILE='sren-%p-%m.profraw'

readonly BIN_DIR=target/debug
readonly COV_FILE=$BIN_DIR/lcov.info
readonly COV_FILE_FIXED=$COV_FILE.fixed
readonly COV_DIR=$BIN_DIR/coverage
readonly COV_INDEX=$COV_DIR/index.html

TEST_OPTS=(
  --no-fail-fast
)

GRCOV_OPTS=(
  --source-dir .
  --binary-path "$BIN_DIR"
  --output-path "$COV_FILE"
  --branch
  --ignore-not-existing
  --ignore "/*"
  --ignore "tests/*"
)

rm -f -- *.profraw
cargo clean
cargo test "${TEST_OPTS[@]}"
grcov . "${GRCOV_OPTS[@]}"
rm -f -- *.profraw

if [[ -x "$(command -v rust-covfix)" ]]; then
  rust-covfix --verbose --output "$COV_FILE_FIXED" "$COV_FILE"
  mv "$COV_FILE_FIXED" "$COV_FILE"
fi

genhtml --output-directory "$COV_DIR" "$COV_FILE"

if [[ -x "$(command -v xdg-open)" ]]; then
  xdg-open "$COV_INDEX"
elif [[ ${MSYSTEM-} =~ ^MINGW(32|64)$ && -x "$(command -v start)" ]]; then
  start "$COV_INDEX"
fi
