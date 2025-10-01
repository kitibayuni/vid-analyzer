#ifndef VIDEO_MODULE_H
#define VIDEO_MODULE_H

#include <opencv2/opencv.hpp>
#include <atomic>
#include <memory>
#include <thread>
#include <mutex>
#include <deque>

extern "C" {
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/opt.h>
}

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
    cv::Mat frame;
    double timestamp;
    bool valid;
    
    FrameBuffer();
    FrameBuffer(FrameBuffer&& other) noexcept;
    FrameBuffer& operator=(FrameBuffer&& other) noexcept;
    
    FrameBuffer(const FrameBuffer&) = delete;
    FrameBuffer& operator=(const FrameBuffer&) = delete;
};

class VideoModule {
private:
    AVFormatContext* formatContext;
    AVCodecContext* codecContext;
    int streamIndex;
    
    enum AVHWDeviceType hwDeviceType;
    AVBufferRef* hwDeviceCtx;
    
    int videoWidth, videoHeight;
    double fps;
    double totalDuration;
    double frameDuration;
    
    std::deque<FrameBuffer> frameQueue;
    mutable std::mutex frameQueueMutex;
    size_t maxQueueSize;
    
    std::mutex currentFrameMutex;
    cv::Mat currentFrame;
    double currentFrameTimestamp;
    
    std::atomic<bool> shouldStop{false};
    std::atomic<bool> isDecoding{false};
    std::atomic<bool> seekRequested{false};
    std::atomic<double> seekTargetTime{0.0};
    std::thread decodeThread;
    
    std::atomic<int> decodedFrames{0};
    std::atomic<int> droppedFrames{0};
    std::atomic<int> renderedFrames{0};

    VideoConfig config;
    int actualThreadCount;
    
    // Adaptive buffering
    std::atomic<bool> isBuffering{false};
    double bufferFullnessThreshold;  // Dynamic based on FPS
    double bufferLowThreshold;       // Dynamic based on FPS
    double decodeAheadTime;          // Dynamic based on FPS
    
    // Internal methods
    bool initializeHardwareAccel();
    bool setupDecoder();
    void decodeThreadFunction();
    void performSeek(double targetTime);
    bool convertAVFrameToMat(AVFrame* frame, cv::Mat& output);
    int detectOptimalThreadCount();
    void calculateDynamicBufferParams();

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