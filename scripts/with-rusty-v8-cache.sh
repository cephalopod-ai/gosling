#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cache_base="${GOSLING_V8_CACHE_DIR:-${XDG_CACHE_HOME:-${HOME}/.cache}/gosling/rusty-v8}"
cache_root="$cache_base"
lock_dir=""
download_tmp=""
archive_tmp=""

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

cleanup() {
  if [[ -n "$download_tmp" ]]; then
    rm -f -- "$download_tmp"
  fi
  if [[ -n "$archive_tmp" ]]; then
    rm -f -- "$archive_tmp"
  fi
  if [[ -n "$lock_dir" && -f "$lock_dir/pid" ]]; then
    local owner
    owner="$(<"$lock_dir/pid")"
    if [[ "$owner" == "$$" ]]; then
      rm -f -- "$lock_dir/pid"
      rmdir "$lock_dir" 2>/dev/null || true
    fi
  fi
}

trap cleanup EXIT

archive_size() {
  stat -f '%z' "$1" 2>/dev/null || stat -c '%s' "$1"
}

archive_valid() {
  local archive="$1"
  [[ -f "$archive" ]] || return 1
  [[ "$(archive_size "$archive")" -ge 10000000 ]] || return 1
  ar -t "$archive" >/dev/null 2>&1
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
  local attempt owner
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
    if [[ -n "$owner" ]] && ! kill -0 "$owner" 2>/dev/null; then
      rm -f -- "$lock_dir/pid"
      rmdir "$lock_dir" 2>/dev/null || true
      continue
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
  sha256_file "$cache_archive" > "$cache_archive.sha256"
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

  cache_archive="$cache_dir/$archive_name"
  if archive_valid "$cache_archive"; then
    if [[ -f "$cache_archive.sha256" ]] && [[ "$(<"$cache_archive.sha256")" != "$(sha256_file "$cache_archive")" ]]; then
      rm -f -- "$cache_archive" "$cache_archive.sha256"
    else
      printf '%s\n' "$cache_archive"
      return
    fi
  fi

  acquire_lock
  if archive_valid "$cache_archive"; then
    printf '%s\n' "$cache_archive"
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
  printf '%s\n' "$cache_archive"
}

if [[ "${1:-}" == '--prepare' ]]; then
  ensure_cache
  exit 0
fi

[[ "$#" -gt 0 ]] || fail 'usage: scripts/with-rusty-v8-cache.sh [--prepare | command ...]'
archive="$(ensure_cache "$@")"
exec env RUSTY_V8_ARCHIVE="$archive" "$@"
