#ifndef MEDIA_CONTROLLER_H
#define MEDIA_CONTROLLER_H

#include "audio_module.h"
#include "video_module.h"
#include <atomic>
#include <memory>
#include <mutex>
#include <string>
#include <chrono>

extern "C" {
#include <libavformat/avformat.h>
}

enum class MediaState {
  STOPPED,
  LOADING,
  READY,
  PLAYING,
  PAUSED,
  SEEKING,
  ERROR
};

struct MediaInfo {
  std::string filename;
  double duration = 0.0;
  bool hasAudio = false;
  bool hasVideo = false;
  int videoWidth = 0;
  int videoHeight = 0;
  double fps = 0.0;
  int audioSampleRate = 0;
  int audioChannels = 0;
  std::string videoCodec;
  std::string audioCodec;
};

struct MediaStats {
  MediaState state = MediaState::STOPPED;
  double currentTime = 0.0;
  AudioStats audioStats;
  VideoStats videoStats;
  double syncOffset = 0.0; // video behind audio (negative = video ahead)
};

class MediaController {
private:
  // Modules
  std::unique_ptr<AudioModule> audioModule;
  std::unique_ptr<VideoModule> videoModule;

  // Separate FFmpeg format contexts for thread safety
  AVFormatContext* audioFormatContext{nullptr};
  AVFormatContext* videoFormatContext{nullptr};
  std::string currentFilename; // Store filename for context creation

  // Media state
  std::atomic<MediaState> currentState{MediaState::STOPPED};
  MediaInfo mediaInfo;
  mutable std::mutex mediaInfoMutex;

  // Stream indices
  int audioStreamIndex{-1};
  int videoStreamIndex{-1};

  // Synchronization
  std::atomic<bool> syncEnabled{true};
  std::atomic<double> maxSyncOffset{0.1}; // 100ms tolerance

  // Video-only playback timing (for when audio is not available)
  std::chrono::steady_clock::time_point playbackStartTime;
  double seekPosition{0.0};  // Current seek position offset
  std::mutex playbackTimeMutex;

  // Thread safety for operations
  std::mutex operationMutex;

  // Internal methods
  bool openFile(const std::string &filename);
  bool findStreams();
  bool loadStreams();
  void closeFile();
  void updateMediaInfo();
  void setState(MediaState newState);
  double calculateSyncOffset() const;

public:
  MediaController();
  ~MediaController();

  // Core functionality
  bool initialize(const AudioConfig &audioConfig = AudioConfig(),
                  const VideoConfig &videoConfig = VideoConfig());
  void shutdown();

  // File operations
  bool loadFile(const std::string &filename);
  void unloadFile();

  // Playback control
  bool play();
  bool pause();
  bool stop();
  bool seek(double timeSeconds);

  // Volume control (delegated to audio module)
  void setVolume(float volume);
  float getVolume() const;

  // Synchronization control
  void enableSync(bool enable);
  bool isSyncEnabled() const;
  void setMaxSyncOffset(double offsetSeconds);
  double getMaxSyncOffset() const;

  // Information queries
  MediaState getState() const;
  MediaInfo getMediaInfo() const;
  MediaStats getStats() const;
  double getCurrentTime() const;
  double getDuration() const;

  // Frame access for UI
  cv::Mat getCurrentVideoFrame();

  // Stream queries
  bool hasAudio() const;
  bool hasVideo() const;

  // Error handling
  std::string getLastError() const;

  // Disable copy/assignment for resource safety
  MediaController(const MediaController &) = delete;
  MediaController &operator=(const MediaController &) = delete;
  MediaController(MediaController &&) = delete;
  MediaController &operator=(MediaController &&) = delete;

private:
  std::string lastError;
  void setError(const std::string &error);
};


#endif // MEDIA_CONTROLLER_H