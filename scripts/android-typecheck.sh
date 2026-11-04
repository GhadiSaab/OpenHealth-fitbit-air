#!/usr/bin/env bash
# Type-checks all Android-only Rust code (cfg(mobile) paths, the vault plugin,
# reminders) for aarch64-linux-android WITHOUT the Android NDK.
#
# C dependencies (SQLite, ring) normally need the NDK compiler. For
# `cargo check` nothing is linked, so a stand-in compiler that emits empty
# objects is enough. This verifies the Rust code compiles for Android; it does
# not build or test an APK (use `cargo tauri android build` with a real NDK).
set -euo pipefail
cd "$(dirname "$0")/.."
stub="$(mktemp -d)"
trap 'rm -rf "$stub"' EXIT
echo 'int airspace_stub_symbol_for_check;' > "$stub/empty.c"
cat > "$stub/cc.sh" <<STUB
#!/usr/bin/env bash
for a in "\$@"; do case "\$a" in *detect_compiler_family.c) exec clang -E "\$a";; esac; done
for a in "\$@"; do [ "\$a" = "-E" ] && exec clang "\$@"; done
for a in "\$@"; do [ "\$a" = "--version" ] && exec clang --version; done
out=""; prev=""
for a in "\$@"; do
  [ "\$prev" = "-o" ] && out="\$a"
  case "\$a" in -o?*) out="\${a#-o}";; -Fo?*) out="\${a#-Fo}";; esac
  prev="\$a"
done
[ -z "\$out" ] && exit 0
exec clang --target=aarch64-linux-android -c "$stub/empty.c" -o "\$out"
STUB
chmod +x "$stub/cc.sh"
rustup target add aarch64-linux-android >/dev/null
CC_aarch64_linux_android="$stub/cc.sh" AR_aarch64_linux_android=ar \
  cargo check -p airspace-tauri --target aarch64-linux-android "$@"
