#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cache_base="${GOSLING_V8_CACHE_DIR:-${XDG_CACHE_HOME:-${HOME}/.cache}/gosling/rusty-v8}"
cache_root="$cache_base"
lock_dir=""
download_tmp=""
archive_tmp=""
lock_pidless_grace_seconds="${GOSLING_V8_LOCK_GRACE_SECONDS:-10}"

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

release_lock() {
  [[ -n "$lock_dir" ]] || return 0
  if [[ -f "$lock_dir/pid" && "$(<"$lock_dir/pid")" == "$$" ]]; then
    rm -f -- "$lock_dir/pid"
    rmdir "$lock_dir" 2>/dev/null || true
  fi
  lock_dir=""
}

cleanup() {
  if [[ -n "$download_tmp" ]]; then
    rm -f -- "$download_tmp"
  fi
  if [[ -n "$archive_tmp" ]]; then
    rm -f -- "$archive_tmp"
  fi
  release_lock
}

trap cleanup EXIT

archive_size() {
  local size
  if size="$(stat -c '%s' "$1" 2>/dev/null)" && [[ "$size" =~ ^[0-9]+$ ]]; then
    printf '%s\n' "$size"
    return
  fi
  if size="$(stat -f '%z' "$1" 2>/dev/null)" && [[ "$size" =~ ^[0-9]+$ ]]; then
    printf '%s\n' "$size"
    return
  fi
  return 1
}

archive_valid() {
  local archive="$1"
  local size
  [[ -f "$archive" ]] || return 1
  size="$(archive_size "$archive")" || return 1
  [[ "$size" -ge 10000000 ]] || return 1
  ar -t "$archive" >/dev/null 2>&1
}

ar_usable() {
  local probe_dir probe_archive status
  probe_dir="$(mktemp -d)" || return 1
  printf 'probe\n' > "$probe_dir/probe.txt"
  probe_archive="$probe_dir/probe.a"
  status=1
  if ar -rc "$probe_archive" "$probe_dir/probe.txt" >/dev/null 2>&1 \
    && ar -t "$probe_archive" >/dev/null 2>&1; then
    status=0
  fi
  rm -rf -- "$probe_dir"
  return "$status"
}

# archive_valid() proves an archive with `ar`, so an `ar` that cannot run at all
# is indistinguishable from a corrupt download unless it is probed separately.
require_ar() {
  if ar_usable; then
    return 0
  fi
  fail "'ar' cannot run, so V8 archive validation is impossible.
On macOS this is usually an unaccepted Xcode license or a stale developer dir:
check 'xcode-select -p', then either run 'sudo xcodebuild -license accept' or
build with DEVELOPER_DIR=/Library/Developer/CommandLineTools."
}

sha256_file() {
  shasum -a 256 "$1" | awk '{print $1}'
}

host_target() {
  rustc -vV | sed -n 's/^host: //p' | head -n 1
}

requested_target() {
  local target
  target="$(host_target)"
  local index
  for ((index = 1; index <= $#; index += 1)); do
    case "${!index}" in
      --target)
        index=$((index + 1))
        [[ "$index" -le "$#" ]] || fail '--target requires a target triple'
        target="${!index}"
        ;;
      --target=*)
        target="${!index#--target=}"
        ;;
    esac
  done
  printf '%s\n' "$target"
}

v8_version() {
  local manifest="$repo_root/vendor/v8/Cargo.toml"
  [[ -f "$manifest" ]] || fail "could not find $manifest"
  sed -n 's/^version = "\([^"]*\)"/\1/p' "$manifest" | head -n 1
}

expected_gzip_sha256() {
  local version="$1"
  local target="$2"
  case "$version:$target" in
    145.0.0:x86_64-unknown-linux-gnu)
      printf '%s\n' '7215753c0c78d141f752d7b993794bae07e18a1dfd466dcaa84fa64e76bacac1'
      ;;
    145.0.0:aarch64-unknown-linux-gnu)
      printf '%s\n' 'e088af62c921512b0c2d963defe836dd4b54621e29e6393b0b384c6cccaa5f26'
      ;;
    145.0.0:x86_64-apple-darwin)
      printf '%s\n' 'd6352e0becfbb1a41f3d820b3724496a70f8fb338e85753669cfcb168cadc21a'
      ;;
    145.0.0:aarch64-apple-darwin)
      printf '%s\n' 'c876b57b27550ab7d81a0ad900d6f382699fdb9a7bba2d5531ab3603b0611ba9'
      ;;
    *)
      return 1
      ;;
  esac
}

acquire_lock() {
  lock_dir="$cache_dir/.lock"
  local attempt owner pidless=0
  for ((attempt = 1; attempt <= 120; attempt += 1)); do
    if mkdir "$lock_dir" 2>/dev/null; then
      printf '%s\n' "$$" > "$lock_dir/pid"
      return
    fi

    owner=""
    if [[ -r "$lock_dir/pid" ]]; then
      owner="$(<"$lock_dir/pid")"
    fi
    if [[ -n "$owner" && ! "$owner" =~ ^[0-9]+$ ]]; then
      owner=""
    fi
    if [[ -n "$owner" ]]; then
      pidless=0
      if ! kill -0 "$owner" 2>/dev/null; then
        rm -f -- "$lock_dir/pid"
        rmdir "$lock_dir" 2>/dev/null || true
        continue
      fi
    else
      # A holder killed between mkdir and the pid write leaves no owner to probe;
      # without this every later run blocks for the full timeout and then fails.
      pidless=$((pidless + 1))
      if ((pidless >= lock_pidless_grace_seconds)); then
        rm -f -- "$lock_dir/pid"
        rmdir "$lock_dir" 2>/dev/null || true
        pidless=0
        continue
      fi
    fi
    sleep 1
  done
  fail "timed out waiting for the V8 cache lock at $lock_dir"
}

copy_to_cache() {
  local source="$1"
  archive_tmp="$cache_dir/.${archive_name}.partial.$$"
  cp "$source" "$archive_tmp"
  archive_valid "$archive_tmp" || fail "the V8 archive at $source is invalid"
  mv -f "$archive_tmp" "$cache_archive"
  archive_tmp=""
  sha256_file "$cache_archive" > "$cache_archive.sha256"
}

download_to_cache() {
  local asset_name="${archive_name}.gz"
  local asset_url="https://github.com/denoland/rusty_v8/releases/download/v${version}/${asset_name}"
  local expected_sha=""
  expected_sha="$(expected_gzip_sha256 "$version" "$target" || true)"
  [[ -n "$expected_sha" ]] || fail "no trusted V8 checksum is recorded for $version / $target"

  download_tmp="$cache_dir/.${asset_name}.partial.$$"
  archive_tmp="$cache_dir/.${archive_name}.partial.$$"
  printf 'Fetching verified V8 archive for %s...\n' "$target" >&2
  curl --fail --location --retry 5 --retry-all-errors --connect-timeout 20 --output "$download_tmp" "$asset_url"
  [[ "$(sha256_file "$download_tmp")" == "$expected_sha" ]] || fail "V8 download checksum mismatch for $asset_url"
  gzip -t "$download_tmp"
  gzip -dc "$download_tmp" > "$archive_tmp"
  archive_valid "$archive_tmp" || fail "downloaded V8 archive failed validation"
  mv -f "$archive_tmp" "$cache_archive"
  archive_tmp=""
  rm -f -- "$download_tmp"
  download_tmp=""
  sha256_file "$cache_archive" > "$cache_archive.sha256"
}

cached_archive_ready() {
  archive_valid "$cache_archive" \
    && [[ -f "$cache_archive.sha256" ]] \
    && [[ "$(<"$cache_archive.sha256")" == "$(sha256_file "$cache_archive")" ]]
}

ensure_cache() {
  version="$(v8_version)"
  [[ -n "$version" ]] || fail 'could not determine the vendored V8 version'
  target="$(requested_target "$@")"
  profile='release'
  if [[ "${V8_FORCE_DEBUG:-}" == 'true' && "$target" != *windows* ]]; then
    profile='debug'
  fi
  feature_suffix="${GOSLING_V8_FEATURE_SUFFIX:-}"
  archive_name="librusty_v8${feature_suffix}_${profile}_${target}.a"
  cache_dir="$cache_root/v${version}"
  mkdir -p "$cache_dir"

  local cache_dir_real target_dir_real cargo_target_dir
  cache_dir_real="$(cd "$cache_dir" && pwd)"
  cargo_target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
  target_dir_real="$(cd "$cargo_target_dir" 2>/dev/null && pwd || printf '%s' "$cargo_target_dir")"
  case "$cache_dir_real" in
    "$target_dir_real"|"$target_dir_real"/*)
      fail 'GOSLING_V8_CACHE_DIR must be outside Cargo target/'
      ;;
  esac

  require_ar

  cache_archive="$cache_dir/$archive_name"
  if cached_archive_ready; then
    return
  fi

  acquire_lock
  if cached_archive_ready; then
    release_lock
    return
  fi

  rm -f -- "$cache_archive" "$cache_archive.sha256"
  local seed_archive="${GOSLING_V8_SEED_ARCHIVE:-}"
  if [[ -n "$seed_archive" ]]; then
    copy_to_cache "$seed_archive"
  else
    local host debug_archive release_archive
    host="$(host_target)"
    debug_archive="$repo_root/target/debug/gn_out/obj/librusty_v8.a"
    release_archive="$repo_root/target/release/gn_out/obj/librusty_v8.a"
    if [[ "$target" == "$host" ]] && archive_valid "$debug_archive"; then
      copy_to_cache "$debug_archive"
    elif [[ "$target" == "$host" ]] && archive_valid "$release_archive"; then
      copy_to_cache "$release_archive"
    else
      download_to_cache
    fi
  fi
  release_lock
}

if [[ "${1:-}" == '--prepare' ]]; then
  ensure_cache
  printf '%s\n' "$cache_archive"
  exit 0
fi

[[ "$#" -gt 0 ]] || fail 'usage: scripts/with-rusty-v8-cache.sh [--prepare | command ...]'
ensure_cache "$@"
exec env RUSTY_V8_ARCHIVE="$cache_archive" "$@"
