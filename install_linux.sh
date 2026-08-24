#!/bin/sh
set -u

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
INSTALLER="$SCRIPT_DIR/scripts/install.py"
LOCAL_UV="$SCRIPT_DIR/.installer/uv/uv"
PYTHON_REQUEST=">=3.10,<3.15"

has_linux_egl() {
    if command -v ldconfig >/dev/null 2>&1 && \
        ldconfig -p 2>/dev/null | grep -F "libEGL.so.1" >/dev/null 2>&1; then
        return 0
    fi

    for library in \
        /lib/libEGL.so.1 \
        /lib64/libEGL.so.1 \
        /usr/lib/libEGL.so.1 \
        /usr/lib64/libEGL.so.1 \
        /lib/*/libEGL.so.1 \
        /usr/lib/*/libEGL.so.1 \
        /usr/local/lib/libEGL.so.1
    do
        if [ -e "$library" ]; then
            return 0
        fi
    done
    return 1
}

print_linux_egl_help() {
    distribution=""
    if [ -r /etc/os-release ]; then
        distribution=$(awk -F= '
            $1 == "ID" || $1 == "ID_LIKE" {
                gsub(/["[:space:]]/, "", $2)
                printf "%s ", tolower($2)
            }
        ' /etc/os-release)
    fi

    printf '%s\n' "InstPlot needs the Linux graphics library libEGL.so.1, but it was not found."
    printf '%s\n' "请先运行适合当前发行版的系统安装命令，然后重新运行 ./install_linux.sh："
    case "$distribution" in
        *ubuntu*|*debian*)
            printf '%s\n' "  sudo apt-get update && sudo apt-get install -y libegl1"
            ;;
        *fedora*|*rhel*|*centos*|*rocky*|*almalinux*)
            printf '%s\n' "  sudo dnf install mesa-libEGL"
            ;;
        *arch*|*manjaro*)
            printf '%s\n' "  sudo pacman -S libglvnd"
            ;;
        *opensuse*|*suse*)
            printf '%s\n' "  sudo zypper install Mesa-libEGL1"
            ;;
        *)
            printf '%s\n' "  请使用系统包管理器安装提供 libEGL.so.1 的软件包。"
            ;;
    esac
    printf '%s\n' "安装器不会自动运行 sudo，也不会修改系统软件。"
}

if ! has_linux_egl; then
    print_linux_egl_help
    exit 2
fi

if [ "${INSTPLOT_FORCE_UV_BOOTSTRAP:-0}" != "1" ] && command -v uv >/dev/null 2>&1; then
    UV=$(command -v uv)
else
    "$SCRIPT_DIR/scripts/bootstrap_uv.sh"
    UV="$LOCAL_UV"
fi

"$UV" run --no-project --python "$PYTHON_REQUEST" "$INSTALLER" --apply "$@"

status=$?
if [ "$status" -ne 0 ]; then
    printf '%s\n' "Installation failed. See .install-logs in the project directory."
    exit "$status"
fi

if ! "$SCRIPT_DIR/.venv/bin/python" "$SCRIPT_DIR/scripts/create_user_launcher.py" --platform linux; then
    printf '%s\n' "Could not create the application-menu launcher; use run_instplot.sh."
    exit 1
fi

if [ "${INSTPLOT_INSTALL_ONLY:-0}" = "1" ]; then
    exit 0
fi

exec "$SCRIPT_DIR/run_instplot.sh"
