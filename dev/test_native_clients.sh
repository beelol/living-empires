#!/bin/sh
set -eu
source_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT HUP INT TERM
mkdir -p "$fixture/repo/dev" "$fixture/bin"
cp "$source_root/dev/native-clients.sh" "$fixture/repo/dev/native-clients.sh"
git -C "$fixture/repo" init -q
git -C "$fixture/repo" add dev/native-clients.sh
git -C "$fixture/repo" -c user.name=Test -c user.email=test@example.invalid commit -qm fixture
revision=$(git -C "$fixture/repo" rev-parse HEAD)
export TEST_ROOT="$fixture"
export PATH="$fixture/bin:$PATH"
cat > "$fixture/bin/curl" <<'SH'
#!/bin/sh
cat "$TEST_ROOT/health.json"
SH
cat > "$fixture/bin/cargo" <<'SH'
#!/bin/sh
set -eu
echo build >> "$TEST_ROOT/builds"
test ! -f dirty.txt
if test -f "$TEST_ROOT/fail-build"; then exit 1; fi
mkdir -p "$CARGO_TARGET_DIR/debug"
cat > "$CARGO_TARGET_DIR/debug/empires" <<'BIN'
#!/bin/sh
printf '%s\n' "$*" >> "$TEST_ROOT/clients"
printf '{"ok":true}\n'
BIN
chmod +x "$CARGO_TARGET_DIR/debug/empires"
if test -f "$TEST_ROOT/change-release"; then
  printf '{"ok":true,"accepting_commands":true,"release":"1111111111111111111111111111111111111111"}' > "$TEST_ROOT/health.json"
fi
SH
chmod +x "$fixture/bin/curl" "$fixture/bin/cargo"
healthy() { printf '{"ok":true,"accepting_commands":true,"release":"%s"}' "$revision" > "$fixture/health.json"; }
fail() { if "$@" > "$fixture/failure.log" 2>&1; then echo "Expected refusal: $*" >&2; exit 1; fi; }
cd "$fixture/repo"
healthy
touch dirty.txt
sh dev/native-clients.sh probe 127.0.0.1:4000 2
test "$(wc -l < "$fixture/builds" | tr -d ' ')" = 1
test "$(wc -l < "$fixture/clients" | tr -d ' ')" = 2
grep -q 'probe-1.json' "$fixture/clients"
grep -q 'probe-2.json' "$fixture/clients"
sh dev/native-clients.sh launch 127.0.0.1:4000 2
grep -q 'slot-1.json' "$fixture/clients"
grep -q 'slot-2.json' "$fixture/clients"
sh dev/native-clients.sh build 127.0.0.1:4000
test "$(wc -l < "$fixture/builds" | tr -d ' ')" = 1
printf '{"ok":false,"accepting_commands":false,"release":"%s"}' "$revision" > "$fixture/health.json"
fail sh dev/native-clients.sh launch 127.0.0.1:4000
healthy
printf 'changed' >> ".local/native-clients/builds/$revision/empires"
fail sh dev/native-clients.sh probe 127.0.0.1:4000
rm -rf .local/native-clients/builds
touch "$fixture/change-release"
fail sh dev/native-clients.sh launch 127.0.0.1:4000
test "$(wc -l < "$fixture/clients" | tr -d ' ')" = 4
rm "$fixture/change-release"
healthy
rm -rf .local/native-clients/builds
touch "$fixture/fail-build"
fail sh dev/native-clients.sh build 127.0.0.1:4000
test ! -e ".local/native-clients/builds/$revision/receipt.json"
rm "$fixture/fail-build"
mkdir .local/native-clients/build.lock
fail sh dev/native-clients.sh build 127.0.0.1:4000
rmdir .local/native-clients/build.lock
fail sh dev/native-clients.sh launch 'bad/address' 2
echo 'Native client helper checks passed'
