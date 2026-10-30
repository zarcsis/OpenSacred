#!/usr/bin/env bash
# Our own minimal FFmpeg for the release archive (#1499): a static ffmpeg.exe for 64-bit Windows,
# cross-built on Linux with MinGW-w64, under the LGPL (no GPL or non-free part enabled), with only
# what opensacred-convert runs: the demuxers and decoders of the original's WAV, MP3, WMV (WMV3,
# WMA v2 and Pro) and Bink (the VTex textures), FLAC, FFV1 and libopus and libsvtav1 (both BSD,
# built static here) to encode, the Ogg, Matroska and WebM muxers, the pipe protocol, and the
# filters that convert the formats. The sources are those of versions.toml, checked by their
# SHA-256.
#
#   tools/ffmpeg/build.sh OUT
#
# OUT gets ffmpeg.exe, FFMPEG.md (the versions, the sources and the configuration), licenses/ and
# sources/, the three source archives as downloaded.
# Needs: x86_64-w64-mingw32-gcc and -g++ (mingw-w64), cmake, ninja, nasm, pkg-config, curl, make.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "${1:?the output directory}" && cd "$1" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
host=x86_64-w64-mingw32
prefix=$work/prefix

# A field of a table of versions.toml.
field() {
    awk -v table="[$1]" -v key="$2" '
        $0 == table { inside = 1; next }
        /^\[/ { inside = 0 }
        inside && $1 == key { value = $3; gsub(/"/, "", value); print value }
    ' "$here/versions.toml"
}

# Downloads a source, checks its SHA-256 and unpacks it into $work/<name>.
fetch() {
    local name=$1 url sum file
    url=$(field "$name" url)
    sum=$(field "$name" sha256)
    file=$work/$(basename "$url")
    curl -fsSL --retry 5 --retry-all-errors -o "$file" "$url"
    echo "$sum  $file" | sha256sum -c -
    mkdir -p "$work/$name"
    tar -xf "$file" -C "$work/$name" --strip-components=1
}

fetch opus
fetch svt-av1
fetch ffmpeg

toolchain=$work/mingw.cmake
cat > "$toolchain" <<EOF
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR x86_64)
set(CMAKE_C_COMPILER $host-gcc)
set(CMAKE_CXX_COMPILER $host-g++)
set(CMAKE_RC_COMPILER $host-windres)
set(CMAKE_FIND_ROOT_PATH /usr/$host $prefix)
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
EOF

# libopus, static; its stack protector would want MinGW's libssp in the static link.
cmake -S "$work/opus" -B "$work/build-opus" -G Ninja -DCMAKE_TOOLCHAIN_FILE="$toolchain" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" -DBUILD_SHARED_LIBS=OFF \
    -DOPUS_BUILD_PROGRAMS=OFF -DOPUS_BUILD_TESTING=OFF -DOPUS_STACK_PROTECTOR=OFF \
    -DOPUS_FORTIFY_SOURCE=OFF
cmake --build "$work/build-opus" --target install

# SVT-AV1's encoder, static.
cmake -S "$work/svt-av1" -B "$work/build-svt" -G Ninja -DCMAKE_TOOLCHAIN_FILE="$toolchain" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" -DBUILD_SHARED_LIBS=OFF \
    -DBUILD_APPS=OFF -DBUILD_TESTING=OFF
cmake --build "$work/build-svt" --target install

# FFmpeg with nothing but what the converter runs; the rawvideo, s16le and wav parts serve the
# workflow's smoke test too.
flags=(
    --target-os=mingw32 --arch=x86_64 --cross-prefix=$host- --prefix="$prefix"
    --pkg-config=pkg-config --pkg-config-flags=--static
    --extra-cflags="-I$prefix/include" --extra-ldflags="-L$prefix/lib -static"
    --disable-everything --disable-autodetect --disable-doc --disable-debug --disable-network
    --disable-ffplay --disable-ffprobe --enable-ffmpeg --enable-w32threads
    --enable-libopus --enable-libsvtav1 --enable-swscale --enable-swresample
    --enable-protocol=file,pipe
    --enable-demuxer=wav,mp3,flac,bink,asf,matroska,rawvideo,s16le
    --enable-decoder=pcm_s16le,pcm_u8,pcm_s24le,pcm_s32le,pcm_f32le,adpcm_ms,adpcm_ima_wav,mp3float,mp3,flac,bink,binkaudio_rdft,binkaudio_dct,wmv3,wmav2,wmapro,ffv1,rawvideo,opus,libopus
    --enable-encoder=flac,libopus,ffv1,libsvtav1,rawvideo,pcm_s16le
    --enable-muxer=flac,ogg,matroska,webm,rawvideo,s16le,wav
    --enable-parser=mpegaudio,flac,opus,av1
    --enable-filter=aresample,aformat,format,scale,anull,null
)
cd "$work/ffmpeg"
PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" ./configure "${flags[@]}"
make -j"$(nproc)"
"$host-strip" -o "$out/ffmpeg.exe" ffmpeg.exe

# The sources as downloaded, which the release offers beside the program (the LGPL, docs/08 §4).
mkdir -p "$out/sources"
for name in ffmpeg opus svt-av1; do
    cp "$work/$(basename "$(field "$name" url)")" "$out/sources/"
done

mkdir -p "$out/licenses"
cp COPYING.LGPLv2.1 "$out/licenses/FFmpeg-COPYING.LGPLv2.1"
cp "$work/opus/COPYING" "$out/licenses/opus-COPYING"
cp "$work/svt-av1/LICENSE.md" "$out/licenses/SVT-AV1-LICENSE.md"
cp "$work/svt-av1/PATENTS.md" "$out/licenses/SVT-AV1-PATENTS.md"
{
    echo "# FFmpeg of the OpenSacred release"
    echo
    echo "ffmpeg.exe is FFmpeg $(field ffmpeg version) under the GNU Lesser General Public License 2.1 or later,"
    echo "built statically with libopus $(field opus version) (BSD) and SVT-AV1 $(field svt-av1 version) (BSD with the"
    echo "AOMedia patent license) by tools/ffmpeg/build.sh of the OpenSacred repository; licenses/ holds their texts."
    echo
    echo "Its sources, unchanged:"
    echo
    for name in ffmpeg opus svt-av1; do
        echo "- $(field "$name" url) (SHA-256 $(field "$name" sha256))"
    done
    echo
    echo "Its configuration:"
    echo
    echo '```'
    printf '%s\n' "${flags[@]}" | sed "s|$prefix|<prefix>|g"
    echo '```'
} > "$out/FFMPEG.md"
ls -l "$out"
