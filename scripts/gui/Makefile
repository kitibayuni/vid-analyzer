# Modular Video Player Makefile
# For simplified structure: src/ directory with module subdirectories

CXX = g++
CXXFLAGS = -std=c++17 -O3 -march=native -Wall -Wextra

TARGET = modular_video_player

# Source directories
SRCDIR = src
BUILDDIR = build
OBJDIR = $(BUILDDIR)/objects

# Source files in subdirectories
AUDIO_SRC = $(SRCDIR)/audio/audio_module.cpp
VIDEO_SRC = $(SRCDIR)/video/video_module.cpp
MEDIA_SRC = $(SRCDIR)/media/media_controller.cpp
UI_SRC = $(SRCDIR)/ui/ui_module.cpp
MAIN_SRC = $(SRCDIR)/main.cpp

# All source files
SOURCES = $(MAIN_SRC) $(AUDIO_SRC) $(VIDEO_SRC) $(MEDIA_SRC) $(UI_SRC)

# Object files (maintain directory structure in build)
OBJECTS = $(OBJDIR)/main.o \
          $(OBJDIR)/audio/audio_module.o \
          $(OBJDIR)/video/video_module.o \
          $(OBJDIR)/media/media_controller.o \
          $(OBJDIR)/ui/ui_module.o

# Header files for dependency tracking
HEADERS = $(SRCDIR)/audio/audio_module.h \
          $(SRCDIR)/video/video_module.h \
          $(SRCDIR)/media/media_controller.h \
          $(SRCDIR)/ui/ui_module.h

# Include paths
INCLUDES = -I$(SRCDIR) -I$(SRCDIR)/audio -I$(SRCDIR)/video -I$(SRCDIR)/media -I$(SRCDIR)/ui

# Package config for dependencies
PKG_CONFIG = pkg-config

# Platform detection
UNAME_S := $(shell uname -s)

# Common libraries
LIBS = -pthread

# OpenCV
OPENCV_CFLAGS = $(shell $(PKG_CONFIG) --cflags opencv4 2>/dev/null || $(PKG_CONFIG) --cflags opencv 2>/dev/null || echo "-I/usr/include/opencv4")
OPENCV_LIBS = $(shell $(PKG_CONFIG) --libs opencv4 2>/dev/null || $(PKG_CONFIG) --libs opencv 2>/dev/null)

# SDL2
SDL2_CFLAGS = $(shell $(PKG_CONFIG) --cflags sdl2 2>/dev/null || echo "-I/usr/include/SDL2")
SDL2_LIBS = $(shell $(PKG_CONFIG) --libs sdl2 2>/dev/null || echo "-lSDL2")

# FFmpeg
FFMPEG_CFLAGS = $(shell $(PKG_CONFIG) --cflags libavformat libavcodec libswresample libswscale libavutil 2>/dev/null)
FFMPEG_LIBS = $(shell $(PKG_CONFIG) --libs libavformat libavcodec libswresample libswscale libavutil 2>/dev/null)

# Platform-specific settings
ifeq ($(UNAME_S),Linux)
    CXXFLAGS += -DLINUX
endif

ifeq ($(UNAME_S),Darwin)
    CXXFLAGS += -DMACOS
endif

ifdef MINGW
    CXXFLAGS += -DWINDOWS
    LIBS += -lcomctl32 -lcomdlg32
endif

# Combine all flags
ALL_CFLAGS = $(CXXFLAGS) $(INCLUDES) $(OPENCV_CFLAGS) $(SDL2_CFLAGS) $(FFMPEG_CFLAGS)
ALL_LIBS = $(OPENCV_LIBS) $(SDL2_LIBS) $(FFMPEG_LIBS) $(LIBS)

# Default target
all: $(TARGET)

# Create necessary directories
$(OBJDIR):
	@mkdir -p $(OBJDIR)
	@mkdir -p $(OBJDIR)/audio
	@mkdir -p $(OBJDIR)/video  
	@mkdir -p $(OBJDIR)/media
	@mkdir -p $(OBJDIR)/ui

# Build target
$(TARGET): $(OBJDIR) $(OBJECTS)
	@echo "Linking $(TARGET)..."
	$(CXX) $(OBJECTS) -o $(TARGET) $(ALL_LIBS)
	@echo "Build complete!"

# Compile individual modules
$(OBJDIR)/main.o: $(SRCDIR)/main.cpp $(HEADERS)
	@echo "Compiling main.cpp..."
	$(CXX) $(ALL_CFLAGS) -c $< -o $@

$(OBJDIR)/audio/audio_module.o: $(SRCDIR)/audio/audio_module.cpp $(SRCDIR)/audio/audio_module.h
	@echo "Compiling audio module..."
	$(CXX) $(ALL_CFLAGS) -c $< -o $@

$(OBJDIR)/video/video_module.o: $(SRCDIR)/video/video_module.cpp $(SRCDIR)/video/video_module.h
	@echo "Compiling video module..."
	$(CXX) $(ALL_CFLAGS) -c $< -o $@

$(OBJDIR)/media/media_controller.o: $(SRCDIR)/media/media_controller.cpp $(SRCDIR)/media/media_controller.h $(SRCDIR)/audio/audio_module.h $(SRCDIR)/video/video_module.h
	@echo "Compiling media controller..."
	$(CXX) $(ALL_CFLAGS) -c $< -o $@

$(OBJDIR)/ui/ui_module.o: $(SRCDIR)/ui/ui_module.cpp $(SRCDIR)/ui/ui_module.h $(SRCDIR)/media/media_controller.h
	@echo "Compiling UI module..."
	$(CXX) $(ALL_CFLAGS) -c $< -o $@

# Clean build files
clean:
	@echo "Cleaning build files..."
	rm -rf $(BUILDDIR) $(TARGET)

# Clean and rebuild
rebuild: clean all

# Debug build
debug: CXXFLAGS += -g -DDEBUG -O0
debug: clean $(TARGET)

# Release build
release: CXXFLAGS += -DNDEBUG
release: clean $(TARGET)

# Performance build
performance: CXXFLAGS += -DNDEBUG -O3 -march=native -mtune=native -flto
performance: clean $(TARGET)

# Check for required dependencies
check-deps:
	@echo "Checking dependencies..."
	@command -v $(CXX) >/dev/null 2>&1 || (echo "C++ compiler not found!" && exit 1)
	@command -v $(PKG_CONFIG) >/dev/null 2>&1 || (echo "pkg-config not found!" && exit 1)
	@$(PKG_CONFIG) --exists opencv4 || $(PKG_CONFIG) --exists opencv || (echo "OpenCV not found!" && exit 1)
	@$(PKG_CONFIG) --exists sdl2 || (echo "SDL2 not found!" && exit 1)
	@$(PKG_CONFIG) --exists libavformat || (echo "FFmpeg libavformat not found!" && exit 1)
	@$(PKG_CONFIG) --exists libavcodec || (echo "FFmpeg libavcodec not found!" && exit 1)
	@$(PKG_CONFIG) --exists libswresample || (echo "FFmpeg libswresample not found!" && exit 1)
	@$(PKG_CONFIG) --exists libswscale || (echo "FFmpeg libswscale not found!" && exit 1)
	@$(PKG_CONFIG) --exists libavutil || (echo "FFmpeg libavutil not found!" && exit 1)
	@echo "All dependencies found!"

# Install dependencies (Ubuntu/Debian)
install-deps-ubuntu:
	@echo "Installing dependencies for Ubuntu/Debian..."
	sudo apt update
	sudo apt install -y build-essential pkg-config
	sudo apt install -y libopencv-dev libsdl2-dev
	sudo apt install -y libavformat-dev libavcodec-dev libswresample-dev libswscale-dev libavutil-dev
	sudo apt install -y zenity clang-format clang-tidy

# Install dependencies (Fedora/CentOS/RHEL)
install-deps-fedora:
	@echo "Installing dependencies for Fedora..."
	sudo dnf install -y gcc-c++ make pkg-config
	sudo dnf install -y opencv-devel SDL2-devel
	sudo dnf install -y ffmpeg-devel
	sudo dnf install -y zenity clang clang-tools-extra

# Install dependencies (macOS)
install-deps-macos:
	@echo "Installing dependencies for macOS..."
	brew install opencv sdl2 ffmpeg pkg-config
	brew install llvm  # for clang-format and clang-tidy

# Run the application
run: $(TARGET)
	./$(TARGET)

# Code formatting and analysis
format:
	@echo "Formatting code..."
	@find $(SRCDIR) -name "*.cpp" -o -name "*.h" | xargs clang-format -i

lint:
	@echo "Running clang-tidy..."
	@find $(SRCDIR) -name "*.cpp" | xargs -I {} clang-tidy {} -- $(ALL_CFLAGS)

# Show build information
info:
	@echo "Build Information:"
	@echo "=================="
	@echo "Compiler: $(CXX)"
	@echo "Flags: $(ALL_CFLAGS)"
	@echo "Libraries: $(ALL_LIBS)"
	@echo "Sources: $(SOURCES)"
	@echo "Target: $(TARGET)"
	@echo "Platform: $(UNAME_S)"

# Help
help:
	@echo "Modular Video Player Build System"
	@echo "================================="
	@echo "Available targets:"
	@echo "  all          - Build the application (default)"
	@echo "  clean        - Remove build files"
	@echo "  rebuild      - Clean and build"
	@echo "  debug        - Build with debug symbols"
	@echo "  release      - Build optimized release version"
	@echo "  performance  - Build with maximum optimization"
	@echo "  check-deps   - Check if all dependencies are installed"
	@echo "  format       - Format code with clang-format"
	@echo "  lint         - Run clang-tidy analysis"
	@echo "  run          - Build and run the application"
	@echo "  info         - Show build configuration"
	@echo "  help         - Show this help message"
	@echo ""
	@echo "Dependency installation:"
	@echo "  install-deps-ubuntu  - Install deps on Ubuntu/Debian"
	@echo "  install-deps-fedora  - Install deps on Fedora/RHEL/CentOS"
	@echo "  install-deps-macos   - Install deps on macOS (requires Homebrew)"

.PHONY: all clean rebuild debug release performance check-deps help info run format lint install-deps-ubuntu install-deps-fedora install-deps-macos