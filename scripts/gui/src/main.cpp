#include "media_controller.h"
#include "ui_module.h"
#include <chrono>
#include <iostream>
#include <thread>

class VideoPlayerApplication {
private:
  MediaController mediaController;
  UIModule uiModule;

  // Application state
  bool running;
  std::string pendingVideoFile;

  // File dialog handling (runs in separate thread to avoid blocking)
  void handleLoadVideo() {
    std::thread([this]() {
      std::string filename = openFileDialog();
      if (!filename.empty()) {
        if (!mediaController.loadFile(filename)) {
          uiModule.showError("Failed to load: " +
                             mediaController.getLastError());
        } else {
          // Resize window to match video dimensions
          auto info = mediaController.getMediaInfo();
          if (info.hasVideo) {
            uiModule.setWindowSize(info.videoWidth, info.videoHeight);
          }
        }
      }
    }).detach();
  }

  void handleTogglePlayPause() {
    MediaState state = mediaController.getState();

    if (state == MediaState::PLAYING) {
      mediaController.pause();
    } else if (state == MediaState::READY || state == MediaState::PAUSED) {
      mediaController.play();
    }
  }

  void handleSeek(double ratio) {
    double duration = mediaController.getDuration();
    if (duration > 0) {
      double targetTime = ratio * duration;
      mediaController.seek(targetTime);
    }
  }

  void handleSeekRelative(double seconds) {
    double currentTime = mediaController.getCurrentTime();
    double duration = mediaController.getDuration();
    double newTime = currentTime + seconds;

    // Clamp to valid range
    if (newTime < 0) newTime = 0;
    if (newTime > duration) newTime = duration;

    mediaController.seek(newTime);
  }

  void handleVolumeChange(float volume) { mediaController.setVolume(volume); }

  void handleVolumeChangeRelative(float delta) {
    float currentVolume = 0.7f; // TODO: Get actual current volume from controller
    float newVolume = currentVolume + delta;

    // Clamp to 0-1 range
    if (newVolume < 0.0f) newVolume = 0.0f;
    if (newVolume > 1.0f) newVolume = 1.0f;

    mediaController.setVolume(newVolume);
  }

  void handleExit() { running = false; }

  void handleLoadCSV() {
    std::thread([this]() {
      // UIModule has the file dialog built-in, we just need to call it
      // But we don't have access to openCSVFileDialog from here
      // So let's add a simpler approach - get filename from UI
      std::string filename;
#ifdef _WIN32
      std::cout << "Please enter CSV file path: ";
      std::getline(std::cin, filename);
#elif defined(__linux__)
      std::string command = "zenity --file-selection --title=\"Select CSV File\" "
                          "--file-filter=\"CSV files | *.csv\" 2>/dev/null";
      FILE *pipe = popen(command.c_str(), "r");
      if (pipe) {
        char buffer[1024];
        if (fgets(buffer, sizeof(buffer), pipe) != nullptr) {
          filename = std::string(buffer);
          if (!filename.empty() && filename.back() == '\n') {
            filename.pop_back();
          }
        }
        pclose(pipe);
      } else {
        std::cout << "Please enter CSV file path: ";
        std::getline(std::cin, filename);
      }
#else
      std::cout << "Please enter CSV file path: ";
      std::getline(std::cin, filename);
#endif

      if (!filename.empty()) {
        // Parse the CSV file using UIModule's parser
        // This is a bit awkward since we're in a lambda, but it works
        if (!uiModule.parseCSVFile(filename)) {
          uiModule.showError("Failed to load CSV file");
        } else {
          std::cout << "CSV loaded successfully. Use Prev/Next buttons to navigate." << std::endl;
          // Optionally seek to the first row's start time
          const CSVRow* row = uiModule.getCurrentRow();
          if (row) {
            std::cout << "First row: rank=" << row->rank
                      << ", type=" << row->type
                      << ", start=" << row->start_time
                      << ", end=" << row->end_time << std::endl;
          }
        }
      }
    }).detach();
  }

  void handleNextRow() {
    uiModule.navigateToRow(1);
    const CSVRow* newRow = uiModule.getCurrentRow();

    if (newRow) {
      std::cout << "Navigated to row " << (uiModule.getCurrentRowIndex() + 1)
                << ": start=" << newRow->start_time
                << ", end=" << newRow->end_time
                << ", type=" << newRow->type << std::endl;

      // Optionally seek to this row's start time
      double duration = mediaController.getDuration();
      if (duration > 0 && newRow->start_time <= duration) {
        mediaController.seek(newRow->start_time);
      }
    }
  }

  void handlePrevRow() {
    uiModule.navigateToRow(-1);
    const CSVRow* row = uiModule.getCurrentRow();

    if (row) {
      std::cout << "Navigated to row " << (uiModule.getCurrentRowIndex() + 1)
                << ": start=" << row->start_time
                << ", end=" << row->end_time
                << ", type=" << row->type << std::endl;

      // Optionally seek to this row's start time
      double duration = mediaController.getDuration();
      if (duration > 0 && row->start_time <= duration) {
        mediaController.seek(row->start_time);
      }
    }
  }

  void handleMarkRow(const std::string& mark) {
    // Mark the current row
    uiModule.markCurrentRow(mark);

    // Auto-advance to next row
    if (uiModule.getCurrentRowIndex() < uiModule.getTotalRows() - 1) {
      handleNextRow();
    } else {
      std::cout << "Reached end of CSV file." << std::endl;
    }
  }

  std::string openFileDialog() {
    // This is platform-specific file dialog implementation
    // For now, using command line input as fallback
    std::string filename;

#ifdef _WIN32
    // Windows file dialog would go here
    std::cout << "Please enter video file path: ";
    std::getline(std::cin, filename);
#elif defined(__linux__)
    // Linux zenity dialog
    std::string command =
        "zenity --file-selection --title=\"Select Video File\" "
        "--file-filter=\"Video files | *.mp4 *.avi *.mov *.mkv *.wmv *.flv "
        "*.webm\" 2>/dev/null";

    FILE *pipe = popen(command.c_str(), "r");
    if (pipe) {
      char buffer[1024];
      if (fgets(buffer, sizeof(buffer), pipe) != nullptr) {
        filename = std::string(buffer);
        if (!filename.empty() && filename.back() == '\n') {
          filename.pop_back();
        }
      }
      pclose(pipe);
    } else {
      std::cout << "Please enter video file path: ";
      std::getline(std::cin, filename);
    }
#else
    std::cout << "Please enter video file path: ";
    std::getline(std::cin, filename);
#endif

    return filename;
  }

public:
  VideoPlayerApplication() : running(false) {}

  bool initialize() {
    std::cout << "\n=== MODULAR HIGH-PERFORMANCE VIDEO PLAYER ===" << std::endl;
    std::cout << "Architecture:" << std::endl;
    std::cout << "- MediaController: Orchestrates audio/video modules"
              << std::endl;
    std::cout << "- AudioModule: SDL2 audio with master clock" << std::endl;
    std::cout << "- VideoModule: FFmpeg decoding with hardware acceleration"
              << std::endl;
    std::cout << "- UIModule: OpenCV-based user interface" << std::endl;
    std::cout << "\nFeatures:" << std::endl;
    std::cout << "- Modular design for easy maintenance and testing"
              << std::endl;
    std::cout << "- Lock-free ring buffers for performance" << std::endl;
    std::cout << "- Audio-driven synchronization" << std::endl;
    std::cout << "- Hardware-accelerated decoding" << std::endl;
    std::cout << "- Cross-platform file dialogs" << std::endl;
    std::cout << "=========================================\n" << std::endl;

    // Initialize MediaController with default configurations
    AudioConfig audioConfig;
    VideoConfig videoConfig;

    if (!mediaController.initialize(audioConfig, videoConfig)) {
      std::cerr << "Failed to initialize MediaController" << std::endl;
      return false;
    }

    // Initialize UI
    UIConfig uiConfig;
    if (!uiModule.initialize(uiConfig)) {
      std::cerr << "Failed to initialize UIModule" << std::endl;
      return false;
    }

    // Set up UI callbacks
    UICallbacks callbacks;
    callbacks.onLoadVideo = [this]() { handleLoadVideo(); };
    callbacks.onTogglePlayPause = [this]() { handleTogglePlayPause(); };
    callbacks.onSeek = [this](double ratio) { handleSeek(ratio); };
    callbacks.onSeekRelative = [this](double seconds) { handleSeekRelative(seconds); };
    callbacks.onVolumeChange = [this](float volume) {
      handleVolumeChange(volume);
    };
    callbacks.onVolumeChangeRelative = [this](float delta) {
      handleVolumeChangeRelative(delta);
    };
    callbacks.onExit = [this]() { handleExit(); };
    callbacks.onLoadCSV = [this]() { handleLoadCSV(); };
    callbacks.onNextRow = [this]() { handleNextRow(); };
    callbacks.onPrevRow = [this]() { handlePrevRow(); };
    callbacks.onMarkRow = [this](const std::string& mark) { handleMarkRow(mark); };

    uiModule.setCallbacks(callbacks);

    running = true;
    return true;
  }

  void run() {
    if (!initialize()) {
        std::cerr << "Failed to initialize application" << std::endl;
        return;
    }

    std::cout << "Application started. Controls:" << std::endl;
    std::cout << "- Click 'Load Video' to open a file" << std::endl;
    std::cout << "- Click 'Play/Pause' to control playback" << std::endl;
    std::cout << "- Click timeline to seek" << std::endl;
    std::cout << "- Adjust volume slider" << std::endl;
    std::cout << "- Press ESC to quit" << std::endl;
    std::cout << "============================\n" << std::endl;

    // Timing
    auto lastFrameTime = std::chrono::steady_clock::now();
    const auto targetFrameTime = std::chrono::microseconds(16667); // ~60 FPS UI

    // Stats printing
    auto lastStatsTime = std::chrono::steady_clock::now();
    const auto statsInterval = std::chrono::seconds(5);

    while (running && uiModule.isWindowOpen()) {
        auto currentTime = std::chrono::steady_clock::now();

        // Get current video frame (synced with audio time ideally)
        cv::Mat videoFrame = mediaController.getCurrentVideoFrame();

        // Get current stats + duration
        MediaStats stats = mediaController.getStats();
        double duration = mediaController.getDuration();

        // Update UI only if enough time has passed
        if (currentTime - lastFrameTime >= targetFrameTime) {
            if (!uiModule.update(videoFrame, stats, duration)) {
                break;
            }
            lastFrameTime = currentTime;
        }

        // Process keyboard with very short timeout
        int key = uiModule.processKeyboard(1);
        if (key == 27) break; // ESC exits

        // Print stats less frequently
        if (currentTime - lastStatsTime >= statsInterval &&
            stats.state == MediaState::PLAYING) {
            printStats(stats);
            lastStatsTime = currentTime;
        }

        // Check for exit request from UI
        if (uiModule.shouldExitApplication()) {
            break;
        }
    }

    std::cout << "Application shutting down..." << std::endl;
    shutdown();
}


  void printStats(const MediaStats &stats) {
    std::cout << "\n=== Playback Statistics ===" << std::endl;
    std::cout << "Time: " << formatTime(stats.currentTime) << std::endl;
    std::cout << "State: " << stateToString(stats.state) << std::endl;

    if (mediaController.hasAudio()) {
      std::cout << "Audio Buffer: "
                << static_cast<int>(stats.audioStats.bufferFullness * 100)
                << "%" << std::endl;
      std::cout << "Audio Underruns: " << stats.audioStats.underruns
                << std::endl;
    }

    if (mediaController.hasVideo()) {
      std::cout << "Video Queue: " << stats.videoStats.queueSize << " frames"
                << std::endl;
      std::cout << "Frames - Decoded: " << stats.videoStats.decodedFrames
                << ", Dropped: " << stats.videoStats.droppedFrames
                << ", Rendered: " << stats.videoStats.renderedFrames
                << std::endl;

      if (stats.videoStats.hardwareAccelEnabled) {
        std::cout << "Hardware Acceleration: " << stats.videoStats.hwAccelType
                  << std::endl;
      }
    }

    if (mediaController.isSyncEnabled()) {
      std::cout << "A/V Sync Offset: " << (stats.syncOffset * 1000) << "ms"
                << std::endl;
    }

    // In the main loop, add this debug output:
    if (stats.state == MediaState::PLAYING) {
        std::cout << "DEBUG: Audio time: " << mediaController.getCurrentTime() 
                  << ", Video queue: " << stats.videoStats.queueSize 
                  << ", Decoded: " << stats.videoStats.decodedFrames << std::endl;
    }
    std::cout << "========================\n" << std::endl;
  }

  std::string formatTime(double seconds) const {
    if (seconds < 0)
      seconds = 0;
    int mins = static_cast<int>(seconds) / 60;
    int secs = static_cast<int>(seconds) % 60;

    std::stringstream ss;
    ss << mins << ":" << std::setfill('0') << std::setw(2) << secs;
    return ss.str();
  }

  std::string stateToString(MediaState state) const {
    switch (state) {
    case MediaState::STOPPED:
      return "Stopped";
    case MediaState::LOADING:
      return "Loading";
    case MediaState::READY:
      return "Ready";
    case MediaState::PLAYING:
      return "Playing";
    case MediaState::PAUSED:
      return "Paused";
    case MediaState::SEEKING:
      return "Seeking";
    case MediaState::ERROR:
      return "Error";
    default:
      return "Unknown";
    }
  }

  void shutdown() {
    mediaController.shutdown();
    uiModule.shutdown();
    std::cout << "Application shutdown complete." << std::endl;
  }
};

int main() {
  try {
    VideoPlayerApplication app;
    app.run();
  } catch (const std::exception &e) {
    std::cerr << "Application error: " << e.what() << std::endl;
    return -1;
  } catch (...) {
    std::cerr << "Unknown application error occurred" << std::endl;
    return -1;
  }

  return 0;
}

/*
=== COMPILATION INSTRUCTIONS ===

To compile this modular video player:

g++ -std=c++11 -O3 -march=native \
    main.cpp \
    audio_module.cpp \
    video_module.cpp \
    media_controller.cpp \
    ui_module.cpp \
    -o modular_video_player \
    `pkg-config --cflags --libs opencv4` \
    `pkg-config --cflags --libs sdl2` \
    `pkg-config --cflags --libs libavformat libavcodec libswresample libavutil`
\ -pthread

Required Files:
- main.cpp (this file)
- audio_module.h/cpp
- video_module.h/cpp
- media_controller.h/cpp
- ui_module.h/cpp

Dependencies:
- OpenCV 4.x
- SDL2
- FFmpeg 4.x+
- C++11 compiler

=== MODULAR ARCHITECTURE BENEFITS ===

1. **Separation of Concerns**:
   - AudioModule: Pure audio handling
   - VideoModule: Pure video handling
   - MediaController: Orchestration and sync
   - UIModule: User interface only

2. **Maintainability**:
   - Each module can be updated independently
   - Clear interfaces between components
   - Easy to locate and fix bugs

3. **Testability**:
   - Each module can be unit tested separately
   - Mock implementations can be created easily
   - Integration testing is more focused

4. **Reusability**:
   - Modules can be used in other applications
   - Easy to swap implementations (e.g., different UI backends)
   - Audio/Video modules could work in headless applications

5. **Scalability**:
   - New features can be added to specific modules
   - Easy to add new media formats or audio backends
   - Threading is contained within each module

6. **Performance**:
   - Same lock-free, high-performance code
   - Better cache locality within modules
   - Easier to optimize individual components

This modular approach transforms the monolithic player into a professional,
maintainable architecture while preserving all performance optimizations.
*/