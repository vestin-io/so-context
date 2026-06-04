#!/usr/bin/env sh
set -eu

REPO="${SO_CONTEXT_REPO:-vestin-io/so-context}"
BIN_NAME="so-context"
INSTALL_DIR="${SO_CONTEXT_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${SO_CONTEXT_VERSION:-latest}"
SKIP_BINARY_DOWNLOAD="${SO_CONTEXT_SKIP_BINARY_DOWNLOAD:-0}"
SOURCE_DIR="${SO_CONTEXT_SOURCE_DIR:-}"

say() {
  printf '%s\n' "$*"
}

fail() {
  say "so-context install: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

normalize_tag() {
  tag="$1"
  case "$tag" in
    v*) printf '%s\n' "$tag" ;;
    *) printf 'v%s\n' "$tag" ;;
  esac
}

detect_target() {
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"

  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *) fail "unsupported architecture: $arch" ;;
  esac

  case "$os" in
    darwin) printf '%s\n' "${arch}-apple-darwin" ;;
    linux) printf '%s\n' "${arch}-unknown-linux-gnu" ;;
    *) fail "unsupported operating system: $os" ;;
  esac
}

download_release_binary() {
  target="$1"
  tmpdir="$2"
  asset="${BIN_NAME}-${target}.tar.gz"

  if [ "$VERSION" = "latest" ]; then
    url="https://github.com/$REPO/releases/latest/download/$asset"
  else
    tag="$(normalize_tag "$VERSION")"
    url="https://github.com/$REPO/releases/download/$tag/$asset"
  fi

  tarball="$tmpdir/$asset"
  if ! curl -fsSL "$url" -o "$tarball"; then
    return 1
  fi

  extract_dir="$tmpdir/extract"
  mkdir -p "$extract_dir"
  tar -xzf "$tarball" -C "$extract_dir"

  bin_path="$(find "$extract_dir" -type f -name "$BIN_NAME" | head -n 1)"
  [ -n "$bin_path" ] || fail "release archive did not contain $BIN_NAME"

  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$bin_path" "$INSTALL_DIR/$BIN_NAME"
  return 0
}

resolve_latest_release_tag() {
  latest_url="$(curl -fsSIL -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest" 2>/dev/null)" ||
    return 1
  tag="${latest_url##*/}"
  [ -n "$tag" ] || return 1
  printf '%s\n' "$tag"
}

resolve_source_archive_url() {
  if [ "$VERSION" = "latest" ]; then
    if tag="$(resolve_latest_release_tag)"; then
      printf '%s\n' "https://github.com/$REPO/archive/refs/tags/$tag.tar.gz"
    else
      say "No GitHub release found; falling back to main branch source archive."
      printf '%s\n' "https://github.com/$REPO/archive/refs/heads/main.tar.gz"
    fi
  else
    tag="$(normalize_tag "$VERSION")"
    printf '%s\n' "https://github.com/$REPO/archive/refs/tags/$tag.tar.gz"
  fi
}

install_from_source() {
  tmpdir="$1"
  need_cmd cargo

  if [ -n "$SOURCE_DIR" ]; then
    src_dir="$SOURCE_DIR"
  else
    need_cmd curl
    need_cmd tar

    src_url="$(resolve_source_archive_url)"
    archive="$tmpdir/source.tar.gz"
    curl -fsSL "$src_url" -o "$archive"

    root_dir="$(tar -tzf "$archive" | head -n 1 | cut -d/ -f1)"
    [ -n "$root_dir" ] || fail "could not determine source archive root"

    tar -xzf "$archive" -C "$tmpdir"
    src_dir="$tmpdir/$root_dir"
  fi

  install_root="$tmpdir/install-root"
  cargo install \
    --locked \
    --path "$src_dir" \
    --root "$install_root" \
    --force

  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$install_root/bin/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
}

main() {
  need_cmd uname
  need_cmd install

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT INT TERM

  installed="0"
  if [ "$SKIP_BINARY_DOWNLOAD" != "1" ]; then
    need_cmd curl
    target="$(detect_target)"
    if download_release_binary "$target" "$tmpdir"; then
      installed="1"
      say "Installed $BIN_NAME from GitHub release."
    else
      say "Release binary unavailable for this platform/version; falling back to cargo build."
    fi
  fi

  if [ "$installed" != "1" ]; then
    install_from_source "$tmpdir"
    say "Installed $BIN_NAME from source with cargo."
  fi

  say
  say "Binary: $INSTALL_DIR/$BIN_NAME"
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
      say "Add $INSTALL_DIR to PATH if needed:"
      say "  export PATH=\"$INSTALL_DIR:\$PATH\""
      ;;
  esac
  say
  say "Next step:"
  say "  $BIN_NAME setup"
}

main "$@"
