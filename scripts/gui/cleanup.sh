#!/usr/bin/env bash
set -e

# Source files for the modular video player
SOURCE_FILES=(
    "src/main.cpp"
    "src/audio/audio_module.cpp"
    "src/video/video_module.cpp"
    "src/media/media_controller.cpp"
    "src/ui/ui_module.cpp"
)

HEADER_FILES=(
    "src/audio/audio_module.h"
    "src/video/video_module.h"
    "src/media/media_controller.h"
    "src/ui/ui_module.h"
)

# All files to process
ALL_FILES=("${SOURCE_FILES[@]}" "${HEADER_FILES[@]}")

# Get include paths from pkg-config
OPENCV_CFLAGS=$(pkg-config --cflags opencv4 2>/dev/null || pkg-config --cflags opencv 2>/dev/null || echo "-I/usr/include/opencv4")
SDL2_CFLAGS=$(pkg-config --cflags sdl2 2>/dev/null || echo "-I/usr/include/SDL2")
FFMPEG_CFLAGS=$(pkg-config --cflags libavformat libavcodec libswresample libavutil 2>/dev/null || echo "")

# Combine all compiler flags
COMPILE_FLAGS="-std=c++11 $OPENCV_CFLAGS $SDL2_CFLAGS $FFMPEG_CFLAGS"

echo "🚀 Starting code cleanup for modular video player..."
echo "📁 Processing ${#ALL_FILES[@]} files..."

# Check if files exist
for file in "${ALL_FILES[@]}"; do
    if [[ ! -f "$file" ]]; then
        echo "⚠️  Warning: $file not found, skipping..."
    fi
done

# 1. Run clang-format on all files
echo ""
echo "🧹 Running clang-format..."
for file in "${ALL_FILES[@]}"; do
    if [[ -f "$file" ]]; then
        echo "  Formatting $file..."
        clang-format -i "$file"
    fi
done

# 2. Run clang-tidy on source files only
echo ""
echo "🔍 Running clang-tidy..."
for file in "${SOURCE_FILES[@]}"; do
    if [[ -f "$file" ]]; then
        echo "  Analyzing $file..."
        clang-tidy "$file" -- $COMPILE_FLAGS 2>/dev/null || {
            echo "    ⚠️  clang-tidy had issues with $file (this is often normal)"
        }
    fi
done

# 3. Run Include-What-You-Use (IWYU)
echo ""
echo "✂️  Running IWYU..."
if command -v include-what-you-use &> /dev/null; then
    mkdir -p build/iwyu_reports
    
    for file in "${SOURCE_FILES[@]}"; do
        if [[ -f "$file" ]]; then
            echo "  Checking includes for $file..."
            filename=$(basename "$file" .cpp)
            include-what-you-use "$file" -- $COMPILE_FLAGS 2>&1 | tee "build/iwyu_reports/${filename}_iwyu.txt" || {
                echo "    ⚠️  IWYU had issues with $file"
            }
        fi
    done
    
    echo "💡 IWYU reports saved to build/iwyu_reports/"
else
    echo "⚠️  IWYU not installed. Install with:"
    echo "    Ubuntu/Debian: sudo apt install iwyu"
    echo "    Fedora: sudo dnf install include-what-you-use"
    echo "    macOS: brew install include-what-you-use"
fi

# 4. Optional: Run cppcheck for static analysis
echo ""
echo "🔬 Running cppcheck (if available)..."
if command -v cppcheck &> /dev/null; then
    mkdir -p build/cppcheck_reports
    cppcheck --enable=all --inconclusive --std=c++11 \
             --suppress=missingIncludeSystem \
             --suppress=unusedFunction \
             src/ 2> build/cppcheck_reports/cppcheck_report.txt || true
    echo "📊 Cppcheck report saved to build/cppcheck_reports/cppcheck_report.txt"
else
    echo "⚠️  cppcheck not available. Install with:"
    echo "    Ubuntu/Debian: sudo apt install cppcheck"
    echo "    Fedora: sudo dnf install cppcheck" 
    echo "    macOS: brew install cppcheck"
fi

# 5. Check for TODO/FIXME comments
echo ""
echo "📝 Checking for TODO/FIXME comments..."
if grep -rn "TODO\|FIXME\|XXX\|HACK" "${ALL_FILES[@]}" 2>/dev/null; then
    echo "💡 Found some TODO/FIXME items above"
else
    echo "✅ No TODO/FIXME comments found"
fi

# 6. Basic code metrics
echo ""
echo "📊 Code metrics:"
total_lines=0
for file in "${ALL_FILES[@]}"; do
    if [[ -f "$file" ]]; then
        lines=$(wc -l < "$file")
        total_lines=$((total_lines + lines))
        echo "  $file: $lines lines"
    fi
done
echo "  Total: $total_lines lines"

echo ""
echo "✅ Cleanup finished!"
echo "📁 Check build/ directory for detailed reports"