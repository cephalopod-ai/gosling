#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
helper="$script_dir/with-rusty-v8-cache.sh"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/gosling-v8-helper-test.XXXXXX")"

cleanup() {
  rm -rf -- "$test_root"
}
trap cleanup EXIT

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

assert_file() {
  [[ -f "$1" ]] || fail "expected file: $1"
}

assert_contains() {
  local output="$1"
  local expected="$2"
  [[ "$output" == *"$expected"* ]] || fail "expected '$expected' in: $output"
}

assert_fails() {
  local expected="$1"
  shift
  local output
  if output="$("$@" 2>&1)"; then
    fail "command unexpectedly passed: $*"
  fi
  assert_contains "$output" "$expected"
}

make_seed_archive() {
  local dir="$1"
  mkdir -p "$dir"
  printf 'const unsigned char payload[11000000] = {1};\n' > "$dir/seed.c"
  cc -c "$dir/seed.c" -o "$dir/seed.o"
  ar rcs "$dir/librusty_v8.a" "$dir/seed.o"
}

host_target() {
  rustc -vV | sed -n 's/^host: //p' | head -n 1
}

printf 'Testing V8 helper with disposable archives...\n'
seed_dir="$test_root/seed"
make_seed_archive "$seed_dir"
seed="$seed_dir/librusty_v8.a"
target="$(host_target)"
case "$target" in
  x86_64-unknown-linux-gnu)
    download_target='aarch64-unknown-linux-gnu'
    ;;
  *)
    download_target='x86_64-unknown-linux-gnu'
    ;;
esac

gnu_stat_bin="$test_root/gnu-stat-bin"
mkdir -p "$gnu_stat_bin"
cat > "$gnu_stat_bin/stat" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  -c)
    shift
    [[ "$1" == '%s' ]] || exit 2
    shift
    wc -c < "$1" | tr -d '[:space:]'
    printf '\n'
    ;;
  -f)
    printf 'File: "%s"\n' "${3:-unknown}"
    ;;
  *)
    exit 2
    ;;
esac
EOF
chmod +x "$gnu_stat_bin/stat"
gnu_archive="$(PATH="$gnu_stat_bin:$PATH" GOSLING_V8_CACHE_DIR="$test_root/gnu-stat-cache" GOSLING_V8_SEED_ARCHIVE="$seed" "$helper" --prepare)"
assert_file "$gnu_archive"

nonnumeric_stat_bin="$test_root/nonnumeric-stat-bin"
mkdir -p "$nonnumeric_stat_bin"
cat > "$nonnumeric_stat_bin/stat" <<'EOF'
#!/usr/bin/env bash
printf 'not numeric\n'
EOF
chmod +x "$nonnumeric_stat_bin/stat"
assert_fails 'is invalid' env PATH="$nonnumeric_stat_bin:$PATH" GOSLING_V8_CACHE_DIR="$test_root/nonnumeric-stat-cache" GOSLING_V8_SEED_ARCHIVE="$seed" "$helper" --prepare

cache="$test_root/cache"
archive="$(GOSLING_V8_CACHE_DIR="$cache" GOSLING_V8_SEED_ARCHIVE="$seed" "$helper" --prepare)"
assert_file "$archive"
assert_file "$archive.sha256"
ar -t "$archive" >/dev/null
[[ "$(cat "$archive.sha256")" == "$(shasum -a 256 "$archive" | awk '{print $1}')" ]] || fail 'seed checksum sidecar mismatch'

mtime_before="$(stat -f '%m' "$archive" 2>/dev/null || stat -c '%Y' "$archive")"
sleep 1
warm_archive="$(GOSLING_V8_CACHE_DIR="$cache" "$helper" --prepare)"
mtime_after="$(stat -f '%m' "$archive" 2>/dev/null || stat -c '%Y' "$archive")"
[[ "$warm_archive" == "$archive" ]] || fail 'warm cache returned a different archive'
[[ "$mtime_after" == "$mtime_before" ]] || fail 'warm cache rewrote the archive'

printf 'corrupt' >> "$archive"
printf 'not-the-archive-hash\n' > "$archive.sha256"
repaired="$(GOSLING_V8_CACHE_DIR="$cache" GOSLING_V8_SEED_ARCHIVE="$seed" "$helper" --prepare)"
[[ "$(shasum -a 256 "$repaired" | awk '{print $1}')" == "$(shasum -a 256 "$seed" | awk '{print $1}')" ]] || fail 'corrupt cache was not repaired'

rm -f "$archive.sha256"
repaired="$(GOSLING_V8_CACHE_DIR="$cache" GOSLING_V8_SEED_ARCHIVE="$seed" "$helper" --prepare)"
assert_file "$repaired.sha256"

invalid_seed="$test_root/invalid.a"
printf 'not an archive\n' > "$invalid_seed"
assert_fails 'is invalid' env GOSLING_V8_CACHE_DIR="$test_root/invalid-cache" GOSLING_V8_SEED_ARCHIVE="$invalid_seed" "$helper" --prepare
assert_fails 'must be outside Cargo target/' env GOSLING_V8_CACHE_DIR="$repo_root/target/v8-helper-test-cache" "$helper" --prepare
assert_fails 'no trusted V8 checksum is recorded' env GOSLING_V8_CACHE_DIR="$test_root/unsupported-cache" "$helper" cargo test --target definitely-unsupported-target

fake_bin="$test_root/fake-bin"
mkdir -p "$fake_bin"
cat > "$fake_bin/curl" <<'EOF'
#!/usr/bin/env bash
printf 'simulated network failure\n' >&2
exit 22
EOF
chmod +x "$fake_bin/curl"
assert_fails 'simulated network failure' env PATH="$fake_bin:$PATH" GOSLING_V8_CACHE_DIR="$test_root/network-cache" "$helper" cargo test --target "$download_target"

checksum_bin="$test_root/checksum-bin"
mkdir -p "$checksum_bin"
cat > "$checksum_bin/curl" <<'EOF'
#!/usr/bin/env bash
output=''
while [[ "$#" -gt 0 ]]; do
  if [[ "$1" == '--output' ]]; then
    output="$2"
    break
  fi
  shift
done
[[ -n "$output" ]] || exit 2
printf 'wrong archive bytes\n' > "$output"
EOF
chmod +x "$checksum_bin/curl"
assert_fails 'checksum mismatch' env PATH="$checksum_bin:$PATH" GOSLING_V8_CACHE_DIR="$test_root/checksum-cache" "$helper" cargo test --target "$download_target"

concurrent_cache="$test_root/concurrent-cache"
for index in 1 2 3 4; do
  GOSLING_V8_CACHE_DIR="$concurrent_cache" GOSLING_V8_SEED_ARCHIVE="$seed" "$helper" --prepare > "$test_root/concurrent-$index.out" &
done
wait
first="$(cat "$test_root/concurrent-1.out")"
for index in 2 3 4; do
  [[ "$(cat "$test_root/concurrent-$index.out")" == "$first" ]] || fail 'concurrent callers returned different paths'
done
assert_file "$first"
assert_file "$first.sha256"
ar -t "$first" >/dev/null

probe="$test_root/probe.sh"
cat > "$probe" <<'EOF'
#!/usr/bin/env bash
[[ -f "${RUSTY_V8_ARCHIVE:-}" ]] || exit 1
printf '%s\n' "$RUSTY_V8_ARCHIVE"
EOF
chmod +x "$probe"
propagated="$(GOSLING_V8_CACHE_DIR="$test_root/propagation-cache" GOSLING_V8_SEED_ARCHIVE="$seed" "$helper" "$probe")"
assert_file "$propagated"

debug_archive="$(V8_FORCE_DEBUG=true GOSLING_V8_CACHE_DIR="$test_root/debug-cache" GOSLING_V8_SEED_ARCHIVE="$seed" "$helper" --prepare)"
assert_contains "$debug_archive" '_debug_'
assert_file "$debug_archive"

printf 'PASS: V8 helper cache, integrity, target, failure, lock, profile, and propagation behavior\n'
