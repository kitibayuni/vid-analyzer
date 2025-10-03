#ifndef AUDIO_MODULE_H
#define AUDIO_MODULE_H

#include <SDL2/SDL.h>
#include <atomic>
#include <chrono>
#include <memory>
#include <mutex>
#include <thread>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>
#include <libavutil/opt.h>
#include <libswresample/swresample.h>
}

struct AudioConfig {
  int bufferSize = 1024;  // Reduced from 2048 for lower latency
  int bufferDurationSeconds = 20;

  AudioConfig() = default;
  AudioConfig(int bufSz, int bufDur)
      : bufferSize(bufSz), bufferDurationSeconds(bufDur) {}
};

struct AudioStats {
  double bufferFullness = 0.0;
  int underruns = 0;
  int overruns = 0;
  double currentTime = 0.0;
  bool isPlaying = false;
};

class AudioModule {
private:
  std::mutex contextMutex;
  // SDL Audio
  SDL_AudioDeviceID audioDevice;
  SDL_AudioSpec audioSpec;

  // FFmpeg contexts
  AVFormatContext *formatContext;
  AVCodecContext *codecContext;
  SwrContext *swrContext;
  int streamIndex;

  // Synchronization and state
  std::atomic<double> clockTime{0.0};
  std::atomic<float> volume{0.7f};
  std::atomic<bool> isPlaying{false};
  std::atomic<bool> shouldStop{false};
  std::atomic<bool> endOfFile{false};
  std::chrono::steady_clock::time_point lastClockUpdate;

  // Threading
  std::thread decodeThread;

  // Statistics
  std::atomic<int> underrunCount{0};
  std::atomic<int> overrunCount{0};

  // Buffer thresholds for queue-based audio
  static constexpr uint32_t MIN_BUFFER_SIZE = 8192;
  static constexpr uint32_t MAX_BUFFER_SIZE = 65536;
  static constexpr uint32_t TARGET_BUFFER_SIZE = 32768;

  std::atomic<bool> audioStarted{false};

  // Thread function
  void decodeThreadFunction();

  // Internal helpers
  bool initializeSDL(int sampleRate, int channels, int bufferSize);
  bool setupResampler();
  void cleanup();

public:
  AudioModule();
  ~AudioModule();

  // Core functionality
  bool initialize(const AudioConfig &config = AudioConfig());
  bool loadStream(AVFormatContext *ctx, int audioStreamIndex);
  void shutdown();

  // Playback control
  void play();
  void pause();
  void stop();
  bool seek(double timeSeconds);

  // Volume control
  void setVolume(float vol); // 0.0 to 1.0
  float getVolume() const;

  // Master clock - this is the authoritative time source
  double getCurrentTime() const;
  void setCurrentTime(double timeSeconds);

  // Status queries
  bool isInitialized() const;
  bool isStreamLoaded() const;
  bool getIsPlaying() const;
  AudioStats getStats() const;

  // Thread safety
  void waitForInitialization();

  // Disable copy/assignment for resource safety
  AudioModule(const AudioModule &) = delete;
  AudioModule &operator=(const AudioModule &) = delete;
  AudioModule(AudioModule &&) = delete;
  AudioModule &operator=(AudioModule &&) = delete;
};

#endif // AUDIO_MODULE_H