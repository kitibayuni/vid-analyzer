#ifndef VIDEO_MODULE_H
#define VIDEO_MODULE_H

#include <opencv2/opencv.hpp>
#include <atomic>
#include <memory>
#include <thread>
#include <mutex>
#include <condition_variable>
#include <deque>
#include <chrono>

extern "C" {
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/opt.h>
}

// Thread-safe queue template for pipeline stages
template<typename T>
class ThreadSafeQueue {
private:
    std::deque<T> queue;
    mutable std::mutex mutex;
    std::condition_variable cv;
    size_t maxSize;
    std::atomic<bool>& stopFlag;

public:
    ThreadSafeQueue(size_t max, std::atomic<bool>& stop)
        : maxSize(max), stopFlag(stop) {}

    bool push(T item, std::chrono::milliseconds timeout = std::chrono::milliseconds(100)) {
        std::unique_lock<std::mutex> lock(mutex);
        if (cv.wait_for(lock, timeout, [this] { return queue.size() < maxSize || stopFlag.load(); })) {
            if (stopFlag.load()) return false;
            queue.push_back(std::move(item));
            cv.notify_one();
            return true;
        }
        return false;
    }

    bool pop(T& item, std::chrono::milliseconds timeout = std::chrono::milliseconds(100)) {
        std::unique_lock<std::mutex> lock(mutex);
        if (cv.wait_for(lock, timeout, [this] { return !queue.empty() || stopFlag.load(); })) {
            if (queue.empty()) return false;
            item = std::move(queue.front());
            queue.pop_front();
            cv.notify_one();
            return true;
        }
        return false;
    }

    size_t size() const {
        std::lock_guard<std::mutex> lock(mutex);
        return queue.size();
    }

    void clear() {
        std::lock_guard<std::mutex> lock(mutex);
        queue.clear();
        cv.notify_all();
    }

    bool waitForMinSize(size_t minSize, std::chrono::milliseconds timeout) {
        std::unique_lock<std::mutex> lock(mutex);
        return cv.wait_for(lock, timeout, [this, minSize] {
            return queue.size() >= minSize || stopFlag.load();
        });
    }

    void notifyAll() {
        cv.notify_all();
    }

    // Drop frames from front matching predicate, return count dropped
    template<typename Predicate>
    int dropFrontWhile(Predicate pred, size_t minKeep = 2) {
        std::lock_guard<std::mutex> lock(mutex);
        int dropped = 0;
        while (queue.size() > minKeep && pred(queue.front())) {
            queue.pop_front();
            dropped++;
        }
        cv.notify_all();
        return dropped;
    }

    // Peek at front item without removing it (requires copy)
    bool peekFront(T& item) const {
        std::lock_guard<std::mutex> lock(mutex);
        if (queue.empty()) return false;
        item = queue.front();  // Copy via copy constructor
        return true;
    }

    // Check if queue is empty
    bool empty() const {
        std::lock_guard<std::mutex> lock(mutex);
        return queue.empty();
    }
};

struct VideoConfig {
    int maxWidth = 1200;
    int maxHeight = 800;
    size_t frameQueueSize = 120;
    bool enableHardwareAccel = true;
    int threadCount = 0;  // 0 = auto-detect, -1 = disable multi-threading

    VideoConfig() = default;
    VideoConfig(int w, int h, size_t qSize, bool hwAccel)
        : maxWidth(w), maxHeight(h), frameQueueSize(qSize), enableHardwareAccel(hwAccel) {}
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
    int decoderThreads = 1;
};

struct FrameBuffer {
    std::shared_ptr<cv::Mat> frame;  // Shared ownership - no expensive cloning!
    double timestamp;
    bool valid;

    FrameBuffer() : frame(nullptr), timestamp(0.0), valid(false) {}
    FrameBuffer(std::shared_ptr<cv::Mat> f, double ts) : frame(f), timestamp(ts), valid(true) {}

    // Move constructors (cheap - just moves shared_ptr)
    FrameBuffer(FrameBuffer&& other) noexcept
        : frame(std::move(other.frame)), timestamp(other.timestamp), valid(other.valid) {
        other.valid = false;
    }
    FrameBuffer& operator=(FrameBuffer&& other) noexcept {
        if (this != &other) {
            frame = std::move(other.frame);
            timestamp = other.timestamp;
            valid = other.valid;
            other.valid = false;
        }
        return *this;
    }

    // Copy constructors (cheap - just copies shared_ptr, not frame data!)
    FrameBuffer(const FrameBuffer& other) = default;
    FrameBuffer& operator=(const FrameBuffer& other) = default;
};

// RAII wrapper for AVPacket
struct PacketHolder {
    AVPacket* packet;

    PacketHolder() : packet(av_packet_alloc()) {}
    explicit PacketHolder(AVPacket* p) : packet(p) {}
    ~PacketHolder() { if (packet) av_packet_free(&packet); }

    PacketHolder(PacketHolder&& other) noexcept : packet(other.packet) {
        other.packet = nullptr;
    }
    PacketHolder& operator=(PacketHolder&& other) noexcept {
        if (this != &other) {
            if (packet) av_packet_free(&packet);
            packet = other.packet;
            other.packet = nullptr;
        }
        return *this;
    }

    PacketHolder(const PacketHolder&) = delete;
    PacketHolder& operator=(const PacketHolder&) = delete;
};

// RAII wrapper for AVFrame
struct FrameHolder {
    AVFrame* frame;

    FrameHolder() : frame(av_frame_alloc()) {}
    explicit FrameHolder(AVFrame* f) : frame(f) {}
    ~FrameHolder() { if (frame) av_frame_free(&frame); }

    FrameHolder(FrameHolder&& other) noexcept : frame(other.frame) {
        other.frame = nullptr;
    }
    FrameHolder& operator=(FrameHolder&& other) noexcept {
        if (this != &other) {
            if (frame) av_frame_free(&frame);
            frame = other.frame;
            other.frame = nullptr;
        }
        return *this;
    }

    FrameHolder(const FrameHolder&) = delete;
    FrameHolder& operator=(const FrameHolder&) = delete;
};

class VideoModule {
private:
    AVFormatContext* formatContext;
    AVCodecContext* codecContext;
    int streamIndex;
    
    enum AVHWDeviceType hwDeviceType;
    AVBufferRef* hwDeviceCtx;
    enum AVPixelFormat hwPixelFormat;

    int videoWidth, videoHeight;
    double fps;
    double totalDuration;
    double frameDuration;
    
    // Pipeline queues with RAII management
    std::unique_ptr<ThreadSafeQueue<PacketHolder>> packetQueue;
    std::unique_ptr<ThreadSafeQueue<FrameBuffer>> frameQueue;

    size_t minPacketQueueSize;  // Minimum packets before decode starts

    std::mutex currentFrameMutex;
    std::shared_ptr<cv::Mat> currentFrame;  // Shared ownership - no cloning needed!
    double currentFrameTimestamp;
    std::atomic<double> lastRequestedTime{0.0};  // Track playback position for decode throttling

    // Thread control
    std::atomic<bool> shouldStop{false};
    std::atomic<bool> isDemuxing{false};
    std::atomic<bool> isDecoding{false};
    std::atomic<bool> seekRequested{false};
    std::atomic<double> seekTargetTime{0.0};

    std::thread demuxThread;
    std::thread decodeThread;

    std::atomic<int> decodedFrames{0};
    std::atomic<int> droppedFrames{0};
    std::atomic<int> renderedFrames{0};

    VideoConfig config;
    int actualThreadCount;
    bool useHardwareDecoding;
    
    // Adaptive buffering
    std::atomic<bool> isBuffering{false};
    double bufferFullnessThreshold;  // Dynamic based on FPS
    double bufferLowThreshold;       // Dynamic based on FPS
    double decodeAheadTime;          // Dynamic based on FPS
    std::mutex bufferingMutex;
    std::condition_variable bufferingCV;

    // AVFrame pool for HW transfers (reuse instead of malloc/free)
    std::deque<AVFrame*> hwFramePool;
    std::mutex hwFramePoolMutex;
    static constexpr size_t MAX_HW_FRAME_POOL_SIZE = 8;

    // Internal methods
    bool initializeHardwareAccel();
    bool setupDecoder();
    void demuxThreadFunction();
    void decodeThreadFunction();
    void performSeek(double targetTime);
    bool convertAVFrameToMat(AVFrame* frame, cv::Mat& output);
    int detectOptimalThreadCount();
    void calculateDynamicBufferParams();

    // AVFrame pool management
    AVFrame* getHWFrame();
    void returnHWFrame(AVFrame* frame);

public:
    VideoModule();
    ~VideoModule();
    
    bool initialize(const VideoConfig& cfg = VideoConfig());
    bool loadStream(AVFormatContext* ctx, int videoStreamIndex);
    void shutdown();
    
    void startDecoding();
    void stopDecoding();
    bool seek(double timeSeconds);
    
    cv::Mat getCurrentFrame(double targetTime);
    cv::Mat getCurrentFrameImmediate();
    
    int getWidth() const { return videoWidth; }
    int getHeight() const { return videoHeight; }
    double getFPS() const { return fps; }
    double getDuration() const { return totalDuration; }
    
    bool isInitialized() const;
    bool isStreamLoaded() const;
    bool getIsDecoding() const;
    VideoStats getStats() const;
    bool isReadyToPlay() const;
    bool needsBuffering() const;
    void waitForBuffer();
    
    VideoModule(const VideoModule&) = delete;
    VideoModule& operator=(const VideoModule&) = delete;
    VideoModule(VideoModule&&) = delete;
    VideoModule& operator=(VideoModule&&) = delete;
};

#endif // VIDEO_MODULE_H