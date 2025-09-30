#ifndef VIDEO_MODULE_H
#define VIDEO_MODULE_H

#include <array>
#include <atomic>
#include <memory>
#include <mutex>
#include <opencv2/opencv.hpp>
#include <thread>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/hwcontext.h>
#include <libavutil/opt.h>
}

// Forward declarations
template <typename T, size_t Size> class LockFreeRingBuffer;

struct VideoConfig {
  int maxWidth = 1200;
  int maxHeight = 800;
  size_t frameQueueSize = 120; // frames
  bool enableHardwareAccel = true;

  VideoConfig() = default;
  VideoConfig(int w, int h, size_t qSize, bool hwAccel)
      : maxWidth(w), maxHeight(h), frameQueueSize(qSize),
        enableHardwareAccel(hwAccel) {}
};

struct VideoStats {
  int decodedFrames = 0;
  int droppedFrames = 0;
  int renderedFrames = 0;
  size_t queueSize = 0;
  double fps = 0.0;
  double currentTimestamp = 0.0;
  bool hardwareAccelEnabled = false;
  std::string hwAccelType = "none";
};

struct FrameBuffer {
  cv::Mat frame;
  double timestamp;
  bool valid;

  FrameBuffer();
  FrameBuffer(FrameBuffer &&other) noexcept;
  FrameBuffer &operator=(FrameBuffer &&other) noexcept;

  // Disable copy constructor and assignment for performance
  FrameBuffer(const FrameBuffer &) = delete;
  FrameBuffer &operator=(const FrameBuffer &) = delete;
};

class VideoModule {
private:
  // FFmpeg contexts
  AVFormatContext *formatContext;
  AVCodecContext *codecContext;
  int streamIndex;

  // Hardware acceleration
  enum AVHWDeviceType hwDeviceType;
  AVBufferRef *hwDeviceCtx;

  // Video properties
  int videoWidth, videoHeight;
  double fps;
  double totalDuration;
  double frameDuration;

  // Frame management
  static const size_t MAX_FRAME_QUEUE_SIZE = 120;
  std::unique_ptr<LockFreeRingBuffer<FrameBuffer, MAX_FRAME_QUEUE_SIZE>>
      frameQueue;

  // Current frame for rendering (protected by mutex)
  std::mutex currentFrameMutex;
  cv::Mat currentFrame;
  double currentFrameTimestamp;

  // Threading
  std::atomic<bool> shouldStop{false};
  std::atomic<bool> isDecoding{false};
  std::atomic<bool> seekRequested{false};
  std::atomic<double> seekTargetTime{0.0};
  std::thread decodeThread;

  // 🔹 Added render thread management
  std::thread renderThread;
  std::atomic<bool> shouldRender{false};

  // Statistics
  std::atomic<int> decodedFrames{0};
  std::atomic<int> droppedFrames{0};
  std::atomic<int> renderedFrames{0};

  // Configuration
  VideoConfig config;

  // Internal methods
  bool initializeHardwareAccel();
  bool setupDecoder();
  void decodeThreadFunction();
  void performSeek(double targetTime);
  bool convertAVFrameToMat(AVFrame *frame, cv::Mat &output);
  void updateCurrentFrame(double targetTime, double tolerance = 0.05);

  // 🔹 New render thread function
  void renderThreadFunction();

public:  // <-- this needs to be INSIDE the class, not outside
  VideoModule();
  ~VideoModule();

  // Core functionality
  bool initialize(const VideoConfig &cfg = VideoConfig());
  bool loadStream(AVFormatContext *ctx, int videoStreamIndex);
  void shutdown();

  // Playback control
  void startDecoding();
  void stopDecoding();
  bool seek(double timeSeconds);

  // Frame access - synchronized with audio master clock
  cv::Mat getCurrentFrame(double targetTime);
  cv::Mat getCurrentFrameImmediate(); // Gets current frame without time sync

  // Properties
  int getWidth() const { return videoWidth; }
  int getHeight() const { return videoHeight; }
  double getFPS() const { return fps; }
  double getDuration() const { return totalDuration; }

  // Status queries
  bool isInitialized() const;
  bool isStreamLoaded() const;
  bool getIsDecoding() const;
  VideoStats getStats() const;

  // Disable copy/assignment for resource safety
  VideoModule(const VideoModule &) = delete;
  VideoModule &operator=(const VideoModule &) = delete;
  VideoModule(VideoModule &&) = delete;
  VideoModule &operator=(VideoModule &&) = delete;
};


// Lock-free ring buffer template (moved to header for template instantiation)
template <typename T, size_t Size> class LockFreeRingBuffer {
private:
  std::array<T, Size> buffer;
  std::atomic<size_t> write_pos{0};
  std::atomic<size_t> read_pos{0};

public:
  bool push(T &&item) {
    size_t current_write = write_pos.load(std::memory_order_relaxed);
    size_t next_write = (current_write + 1) % Size;

    if (next_write == read_pos.load(std::memory_order_acquire)) {
      return false; // Buffer full
    }

    buffer[current_write] = std::move(item);
    write_pos.store(next_write, std::memory_order_release);
    return true;
  }

  bool pop(T &item) {
    size_t current_read = read_pos.load(std::memory_order_relaxed);

    if (current_read == write_pos.load(std::memory_order_acquire)) {
      return false; // Buffer empty
    }

    item = std::move(buffer[current_read]);
    read_pos.store((current_read + 1) % Size, std::memory_order_release);
    return true;
  }

  size_t size() const {
    size_t write = write_pos.load(std::memory_order_relaxed);
    size_t read = read_pos.load(std::memory_order_relaxed);
    return (write >= read) ? (write - read) : (Size - read + write);
  }

  bool empty() const {
    return read_pos.load(std::memory_order_relaxed) ==
           write_pos.load(std::memory_order_relaxed);
  }

  void clear() {
    read_pos.store(0, std::memory_order_relaxed);
    write_pos.store(0, std::memory_order_relaxed);
  }
};

#endif // VIDEO_MODULE_H