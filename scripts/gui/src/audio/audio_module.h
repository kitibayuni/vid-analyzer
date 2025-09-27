#ifndef AUDIO_MODULE_H
#define AUDIO_MODULE_H

#include <SDL2/SDL.h>
#include <atomic>
#include <chrono>
#include <memory>
#include <thread>
#include <mutex>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>
#include <libavutil/opt.h>
#include <libswresample/swresample.h>
}

// Forward declarations
class CircularAudioBuffer;

struct AudioConfig {
  int sampleRate = 44100;
  int channels = 2;
  int bufferSize = 2048;
  int bufferDurationSeconds = 20;

  AudioConfig() = default;
  AudioConfig(int rate, int ch, int bufSz, int bufDur)
      : sampleRate(rate), channels(ch), bufferSize(bufSz),
        bufferDurationSeconds(bufDur) {}
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
  std::unique_ptr<CircularAudioBuffer> audioBuffer;

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
  std::chrono::steady_clock::time_point lastClockUpdate;

  // Threading
  std::thread decodeThread;

  // Statistics
  std::atomic<int> underrunCount{0};
  std::atomic<int> overrunCount{0};

  // Preallocated buffers for performance
  struct AudioBuffers {
    uint8_t **output_data;
    int output_linesize;
    int max_samples;

    AudioBuffers();
    ~AudioBuffers();
    bool allocate(int samples, int channels);
  } audioBuffers;

  // Static callback for SDL
  static void audioCallback(void *userdata, Uint8 *stream, int len);

  // Thread function
  void decodeThreadFunction();

  // Internal helpers
  bool initializeSDL(const AudioConfig &config);
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