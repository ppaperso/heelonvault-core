#!/usr/bin/env bash
set -euo pipefail

if [[ $# -eq 0 ]]; then
    echo "Usage: bash scripts/run-wayland-tests.sh COMMAND [ARG...]" >&2
    exit 2
fi
for tool in labwc wayland-info wlr-randr timeout; do
    if ! command -v "$tool" >/dev/null; then
        echo "Missing Wayland test dependency: $tool (install labwc, wayland-utils, wlr-randr and coreutils)." >&2
        exit 1
    fi
done

runtime="$(mktemp -d -t heelonvault-wayland.XXXXXXXX)"
chmod 700 "$runtime"
compositor_pid=""
cleanup() {
    result=$?
    trap - EXIT
    if [[ -n "$compositor_pid" ]]; then
        if kill -0 "$compositor_pid" 2>/dev/null; then
            kill "$compositor_pid"
        fi
        wait "$compositor_pid" || true
    fi
    if [[ $result -ne 0 ]]; then
        echo "Wayland qualification failed (exit $result). Compositor log:" >&2
        tail -80 "$runtime/compositor.log" >&2
    fi
    rm -rf -- "$runtime"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
export XDG_RUNTIME_DIR="$runtime"
unset DISPLAY WAYLAND_DISPLAY
mkdir "$runtime/config"

# Never import this private display into the user's D-Bus/systemd environment.
WLR_BACKENDS=headless WLR_HEADLESS_OUTPUTS=1 WLR_RENDERER=pixman \
    LABWC_UPDATE_ACTIVATION_ENV=0 labwc --config-dir "$runtime/config" \
    >"$runtime/compositor.log" 2>&1 &
compositor_pid=$!
ready=false
deadline=$((SECONDS + 10))
while ((SECONDS < deadline)); do
    if ! kill -0 "$compositor_pid" 2>/dev/null; then
        echo "Compositor exited before the Wayland display was ready." >&2
        exit 1
    fi
    for socket in "$runtime"/wayland-*; do
        if [[ -S "$socket" ]]; then
            export WAYLAND_DISPLAY="${socket##*/}"
            break
        fi
    done
    if [[ -n "${WAYLAND_DISPLAY:-}" ]] && timeout 1s wayland-info >"$runtime/protocols.log" 2>&1 \
        && grep -q "interface: 'wl_seat'" "$runtime/protocols.log"; then
        ready=true
        break
    fi
    sleep 0.1
done
if [[ "$ready" != true ]]; then
    echo "Wayland display did not become responsive within 10 seconds." >&2
    exit 1
fi
wlr-randr --output HEADLESS-1 --custom-mode 1920x1600@60Hz

export GDK_BACKEND=wayland G_DEBUG=fatal-warnings GTK_IM_MODULE=simple GSK_RENDERER=cairo
echo "Qualifying on headless Wayland: fatal warnings, simple input, Cairo; X11 is not certified."
"$@"
