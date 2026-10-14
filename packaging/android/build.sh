#!/usr/bin/env bash
# Development APK, no Java/Kotlin sources or Gradle required.
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK directory}"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}"
ndk_bin="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
build_tools="$ANDROID_HOME/build-tools/35.0.0"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ndk_bin/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export AR_aarch64_linux_android="$ndk_bin/llvm-ar"
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_INCREMENTAL=0
cargo build --locked -p photocraft-android --target aarch64-linux-android
out="${CARGO_TARGET_DIR:-target}/android-apk"
mkdir -p "$out/lib/arm64-v8a" "$out/res/drawable-nodpi"
cp assets/app-icon/hicolor/256x256/apps/ai.storyteller.photocraft.png "$out/res/drawable-nodpi/icon.png"
"$ndk_bin/llvm-strip" --strip-debug -o "$out/lib/arm64-v8a/libphotocraft_android.so" "${CARGO_TARGET_DIR:-target}/aarch64-linux-android/debug/libphotocraft_android.so"
"$build_tools/aapt" package -f -M packaging/android/AndroidManifest.xml -S "$out/res" -I "$ANDROID_HOME/platforms/android-35/android.jar" -F "$out/unaligned.apk"
(cd "$out" && zip -q unaligned.apk lib/arm64-v8a/libphotocraft_android.so)
"$build_tools/zipalign" -f -P 16 4 "$out/unaligned.apk" "$out/unsigned.apk"
key="${PHOTOCRAFT_ANDROID_KEYSTORE:-$HOME/.android/photocraft-debug.keystore}"
if [[ ! -f "$key" ]]; then
    mkdir -p "$(dirname "$key")"
    keytool -genkeypair -keystore "$key" -storepass android -keypass android -alias androiddebugkey -dname 'CN=PhotoCraft Android Debug' -keyalg RSA -keysize 2048 -validity 10000
fi
"$build_tools/apksigner" sign --ks "$key" --ks-key-alias androiddebugkey --ks-pass pass:android --key-pass pass:android --out "$out/PhotoCraft.apk" "$out/unsigned.apk"
"$build_tools/apksigner" verify "$out/PhotoCraft.apk"
echo "APK: $out/PhotoCraft.apk"
