#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
INSTALLER_ROOT="$PROJECT_ROOT/.installer"
UV_ROOT="$INSTALLER_ROOT/uv"
UV="$UV_ROOT/uv"
INSTALLER_URL="https://astral.sh/uv/0.12.5/install.sh"
INSTALLER_SHA256="504511fbbbd811aeaba6738abc79408956b6c7da0ca35437b3dcc24a41efc111"

for directory in "$INSTALLER_ROOT" "$UV_ROOT"; do
    if [ -L "$directory" ] || { [ -e "$directory" ] && [ ! -d "$directory" ]; }; then
        printf '%s\n' "Unsafe uv bootstrap directory: $directory" >&2
        exit 1
    fi
done

if [ -x "$UV" ] && [ ! -L "$UV" ]; then
    exit 0
fi
if [ -e "$UV" ]; then
    printf '%s\n' "Unsafe uv bootstrap executable: $UV" >&2
    exit 1
fi

mkdir -p "$UV_ROOT"
temporary=$(mktemp "${TMPDIR:-/tmp}/instplot-uv-installer.XXXXXX")
trap 'rm -f "$temporary"' EXIT HUP INT TERM

if command -v curl >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -LsSf "$INSTALLER_URL" -o "$temporary"
elif command -v wget >/dev/null 2>&1; then
    wget -qO "$temporary" "$INSTALLER_URL"
else
    printf '%s\n' "curl or wget is required to download the verified uv bootstrap." >&2
    exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$temporary" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$temporary" | awk '{print $1}')
elif command -v openssl >/dev/null 2>&1; then
    actual=$(openssl dgst -sha256 "$temporary" | awk '{print $NF}')
else
    printf '%s\n' "A SHA-256 tool is required to verify the uv bootstrap." >&2
    exit 1
fi

if [ "$actual" != "$INSTALLER_SHA256" ]; then
    printf '%s\n' "uv installer checksum mismatch; refusing to execute it." >&2
    exit 1
fi

UV_UNMANAGED_INSTALL="$UV_ROOT" UV_NO_MODIFY_PATH=1 sh "$temporary"
if [ ! -x "$UV" ] || [ -L "$UV" ]; then
    printf '%s\n' "Verified uv bootstrap did not create the expected executable." >&2
    exit 1
fi
