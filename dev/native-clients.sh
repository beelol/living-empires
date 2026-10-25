#!/bin/sh
# Build the serving revision, never dirty source, and join with independent slots.
set -eu
die() { printf '%s\n' "$*" >&2; exit 1; }
action=${1:-launch}
server=${2:-127.0.0.1:4000}
count=${3:-2}
case "$action" in build|launch|probe) ;; *) die 'Usage: sh dev/native-clients.sh [build|launch|probe] [HOST:PORT] [1|2]' ;; esac
case "$count" in 1|2) ;; *) die 'Client count must be 1 or 2' ;; esac
# Restrict addresses to DNS/IPv4 or bracketed IPv6 and numeric port. Never URLs.
printf '%s\n' "$server" | grep -Eq '^([A-Za-z0-9][A-Za-z0-9.-]*|\[[0-9a-fA-F:]+\]):[0-9]{1,5}$' || die 'Expected HOST:PORT'
port=${server##*:}
test "$port" -ge 1 && test "$port" -le 65535 || die 'Port must be 1..65535'
for tool in git cargo curl jq tar shasum; do command -v "$tool" >/dev/null || die "Missing required tool: $tool"; done
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cache="$root/.local/native-clients"
mkdir -p "$cache/builds" "$cache/snapshots" "$cache/sessions"
health() {
  curl --fail --silent --show-error --connect-timeout 3 --max-time 5 "http://$server/health" |
    jq -er 'select(.ok == true and .accepting_commands == true) | .release | select(type == "string" and test("^[0-9a-f]{40}$"))'
}
revision=$(health) || die 'Host is unavailable, gated, or has invalid release metadata'
git -C "$root" cat-file -e "$revision^{commit}" 2>/dev/null || die "Release $revision is not in this clone. Fetch origin/main through your normal integration workflow and retry."
build="$cache/builds/$revision"
binary="$build/empires"
digest() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
lock_owned=false
children=''
temporary=''
cleanup() {
  if test "$lock_owned" = true; then rmdir "$cache/build.lock" 2>/dev/null || true; fi
  if test -n "$temporary"; then rm -rf "$temporary"; fi
}
stop_children() { for pid in $children; do kill "$pid" 2>/dev/null || true; done; }
trap cleanup EXIT
trap 'stop_children; exit 130' INT
trap 'stop_children; exit 143' TERM HUP
mkdir "$cache/build.lock" 2>/dev/null || die 'Another client build owns the build lock. Wait for it; do not remove a live lock.'
lock_owned=true
if test -f "$build/receipt.json"; then
  test -x "$binary" || die 'Cached client binary is missing or not executable'
  recorded_revision=$(jq -er '.revision' "$build/receipt.json")
  recorded_hash=$(jq -er '.sha256' "$build/receipt.json")
  test "$recorded_revision" = "$revision" && test "$recorded_hash" = "$(digest "$binary")" || die 'Cached client changed after build; preserve it for inspection and use a new cache'
else
  test ! -e "$build" || die 'Incomplete client build exists; preserve it for inspection before retrying'
  temporary=$(mktemp -d "$cache/building.XXXXXX")
  mkdir "$temporary/source" "$temporary/result" "$temporary/result/assets"
  git -C "$root" archive "$revision" > "$temporary/source.tar"
  tar -xf "$temporary/source.tar" -C "$temporary/source"
  # Archive timestamps can predate cached dependency information. Force Cargo to
  # inspect this source revision while retaining compiled third-party crates.
  find "$temporary/source" -type f -exec touch {} +
  printf 'Building native clients for server release %s\n' "$revision"
  (
    cd "$temporary/source"
    CARGO_TARGET_DIR="$cache/target" cargo build --locked -p living-empires --bin empires
  )
  cp "$cache/target/debug/empires" "$temporary/result/empires"
  chmod +x "$temporary/result/empires"
  jq -n --arg revision "$revision" --arg sha256 "$(digest "$temporary/result/empires")" '{revision:$revision,sha256:$sha256}' > "$temporary/result/receipt.json"
  # Keep the exact committed assets alongside the executable's launch directory.
  if test -d "$temporary/source/game/assets"; then cp -R "$temporary/source/game/assets/." "$temporary/result/assets/"; fi
  mv "$temporary/result" "$build"
  rm -rf "$temporary"
  temporary=''
fi
mkdir -p "$build/assets"
rmdir "$cache/build.lock"
lock_owned=false
current=$(health) || die 'Host became unavailable during client preparation; retry when healthy'
test "$current" = "$revision" || die 'Host updated during client preparation. Rerun to build the newly serving revision.'
printf 'Native client ready: %s\n' "$revision"
test "$action" != build || exit 0
server_key=$(printf '%s' "$server" | shasum -a 256 | cut -d ' ' -f 1)
sessions="$cache/sessions/$server_key"
mkdir -p "$sessions"
slot=1
while test "$slot" -le "$count"; do
  if test "$action" = probe; then
    # The probe JSON includes a session capability; keep raw output private.
    umask 077
    "$binary" probe-client "$server" --session "$sessions/probe-$slot.json" --timeout 20 > "$sessions/probe-$slot-result.json"
    printf 'Client %s connected successfully (private receipt saved).\n' "$slot"
  else
    executable="$binary"
    if test "$(uname -s)" = Darwin; then
      # A bundle gives each macOS client an inspectable application identity.
      app="$cache/apps/$revision/Client-$server_key-$slot.app"
      if test ! -d "$app"; then
        mkdir -p "$app/Contents/MacOS"
        ln "$binary" "$app/Contents/MacOS/empires"
        ln -s "$build/assets" "$app/Contents/MacOS/assets"
        cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>empires</string>
<key>CFBundleIdentifier</key><string>dev.livingempires.client.s$server_key.slot$slot</string>
<key>CFBundleName</key><string>Living Empires Client $slot</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
      fi
      cmp "$binary" "$app/Contents/MacOS/empires" >/dev/null || die 'Client application differs from the verified binary'
      executable="$app/Contents/MacOS/empires"
    fi
    (cd "$build" && exec "$executable" join "$server" --session "$sessions/slot-$slot.json") &
    children="$children $!"
  fi
  slot=$((slot + 1))
done
for pid in $children; do wait "$pid" || { stop_children; exit 1; }; done
