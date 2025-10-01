#include "video_module.h"
#include <iostream>
#include <algorithm>
#include <limits>

FrameBuffer::FrameBuffer() : timestamp(0.0), valid(false) {}

FrameBuffer::FrameBuffer(FrameBuffer&& other) noexcept 
    : frame(std::move(other.frame)), timestamp(other.timestamp), valid(other.valid) {
    other.valid = false;
}

FrameBuffer& FrameBuffer::operator=(FrameBuffer&& other) noexcept {
    if (this != &other) {
        frame = std::move(other.frame);
        timestamp = other.timestamp;
        valid = other.valid;
        other.valid = false;
    }
    return *this;
}

VideoModule::VideoModule() 
    : formatContext(nullptr), codecContext(nullptr), streamIndex(-1),
      hwDeviceType(AV_HWDEVICE_TYPE_NONE), hwDeviceCtx(nullptr),
      videoWidth(0), videoHeight(0), fps(30.0), totalDuration(0.0), frameDuration(1.0/30.0),
      maxQueueSize(120), currentFrameTimestamp(0.0) {
}

VideoModule::~VideoModule() {
    shutdown();
}

bool VideoModule::initialize(const VideoConfig& cfg) {
    std::cout << "Initializing VideoModule..." << std::endl;
    config = cfg;
    maxQueueSize = config.frameQueueSize;
    std::cout << "VideoModule initialized successfully" << std::endl;
    return true;
}

bool VideoModule::initializeHardwareAccel() {
    const char* hw_types[] = {"vaapi", "nvdec", "dxva2", "videotoolbox"};
    
    for (const char* type_name : hw_types) {
        AVHWDeviceType type = av_hwdevice_find_type_by_name(type_name);
        if (type != AV_HWDEVICE_TYPE_NONE) {
            if (av_hwdevice_ctx_create(&hwDeviceCtx, type, nullptr, nullptr, 0) >= 0) {
                hwDeviceType = type;
                std::cout << "Hardware acceleration enabled: " << type_name << std::endl;
                return true;
            }
        }
    }
    
    std::cout << "Hardware acceleration not available" << std::endl;
    return false;
}

bool VideoModule::loadStream(AVFormatContext* ctx, int videoStreamIndex) {
    if (!ctx || videoStreamIndex < 0 || videoStreamIndex >= static_cast<int>(ctx->nb_streams)) {
        std::cerr << "VideoModule: Invalid parameters" << std::endl;
        return false;
    }
    
    AVCodecParameters* codecpar = ctx->streams[videoStreamIndex]->codecpar;
    if (codecpar->codec_type != AVMEDIA_TYPE_VIDEO) {
        std::cerr << "VideoModule: Not a video stream" << std::endl;
        return false;
    }
    
    formatContext = ctx;
    streamIndex = videoStreamIndex;
    
    if (!setupDecoder()) {
        std::cerr << "Failed to setup video decoder" << std::endl;
        return false;
    }
    
    AVStream* videoStream = formatContext->streams[streamIndex];
    fps = av_q2d(videoStream->r_frame_rate);
    if (fps <= 0 || fps > 120) fps = 30.0;
    frameDuration = 1.0 / fps;
    
    totalDuration = formatContext->duration / static_cast<double>(AV_TIME_BASE);
    
    videoWidth = codecContext->width;
    videoHeight = codecContext->height;
    
    {
        std::lock_guard<std::mutex> lock(frameQueueMutex);
        frameQueue.clear();
    }
    decodedFrames = 0;
    droppedFrames = 0;
    renderedFrames = 0;
    
    std::cout << "Video loaded: " << videoWidth << "x" << videoHeight 
              << " @ " << fps << "fps" << std::endl;
    
    return true;
}

bool VideoModule::setupDecoder() {
    AVCodecParameters* codecParams = formatContext->streams[streamIndex]->codecpar;
    const AVCodec* codec = avcodec_find_decoder(codecParams->codec_id);
    
    if (!codec) return false;
    
    codecContext = avcodec_alloc_context3(codec);
    if (!codecContext) return false;
    
    if (avcodec_parameters_to_context(codecContext, codecParams) < 0) {
        avcodec_free_context(&codecContext);
        return false;
    }
    
    if (hwDeviceCtx) {
        codecContext->hw_device_ctx = av_buffer_ref(hwDeviceCtx);
    }
    
    codecContext->error_concealment = FF_EC_GUESS_MVS | FF_EC_DEBLOCK;
    
    if (avcodec_open2(codecContext, codec, nullptr) < 0) {
        avcodec_free_context(&codecContext);
        return false;
    }
    
    return true;
}

void VideoModule::startDecoding() {
    if (!isStreamLoaded() || isDecoding) return;
    
    shouldStop = false;
    isDecoding = true;
    isBuffering = true;
    
    decodeThread = std::thread(&VideoModule::decodeThreadFunction, this);
    
    std::cout << "Preloading video buffer..." << std::endl;
    
    while (isBuffering && !shouldStop) {
        size_t queueSize;
        {
            std::lock_guard<std::mutex> lock(frameQueueMutex);
            queueSize = frameQueue.size();
        }
        
        double fullness = static_cast<double>(queueSize) / maxQueueSize;
        
        if (fullness >= bufferFullnessThreshold) {
            isBuffering = false;
            std::cout << "Buffer ready: " << queueSize << " frames" << std::endl;
        } else {
            std::this_thread::sleep_for(std::chrono::milliseconds(50));
        }
    }
    
    std::cout << "Video decoding started" << std::endl;
}

void VideoModule::stopDecoding() {
    if (!isDecoding) return;
    
    isDecoding = false;
    shouldStop = true;
    
    if (decodeThread.joinable()) {
        decodeThread.join();
    }
    
    std::cout << "Video decoding stopped" << std::endl;
}

void VideoModule::decodeThreadFunction() {
    AVPacket* packet = av_packet_alloc();
    AVFrame* frame = av_frame_alloc();
    
    while (!shouldStop && isDecoding) {
        if (seekRequested.load()) {
            performSeek(seekTargetTime.load());
            seekRequested = false;
        }
        
        size_t queueSize;
        {
            std::lock_guard<std::mutex> lock(frameQueueMutex);
            queueSize = frameQueue.size();
        }
        
        // Decode aggressively - keep queue as full as possible
        if (queueSize < maxQueueSize) {
            if (av_read_frame(formatContext, packet) >= 0) {
                if (packet->stream_index == streamIndex) {
                    if (avcodec_send_packet(codecContext, packet) >= 0) {
                        while (avcodec_receive_frame(codecContext, frame) >= 0) {
                            if (frame->data[0]) {
                                FrameBuffer fb;
                                if (convertAVFrameToMat(frame, fb.frame)) {
                                    AVStream* stream = formatContext->streams[streamIndex];
                                    if (frame->pts != AV_NOPTS_VALUE) {
                                        fb.timestamp = frame->pts * av_q2d(stream->time_base);
                                    } else {
                                        fb.timestamp = decodedFrames.load() / fps;
                                    }
                                    fb.valid = true;
                                    
                                    {
                                        std::lock_guard<std::mutex> lock(frameQueueMutex);
                                        frameQueue.push_back(std::move(fb));
                                        decodedFrames++;
                                    }
                                }
                            }
                        }
                    }
                }
                av_packet_unref(packet);
            } else {
                // EOF
                isDecoding = false;
                break;
            }
        } else {
            std::this_thread::sleep_for(std::chrono::milliseconds(5));
        }
    }
    
    av_frame_free(&frame);
    av_packet_free(&packet);
}

bool VideoModule::convertAVFrameToMat(AVFrame* frame, cv::Mat& output) {
    if (!frame || !frame->data[0] || frame->format != AV_PIX_FMT_YUV420P) {
        return false;
    }
    
    int y_size = frame->width * frame->height;
    int uv_size = (frame->width / 2) * (frame->height / 2);
    
    std::vector<uint8_t> yuv_buffer(y_size + 2 * uv_size);
    
    for (int i = 0; i < frame->height; i++) {
        memcpy(yuv_buffer.data() + i * frame->width,
               frame->data[0] + i * frame->linesize[0],
               frame->width);
    }
    
    for (int i = 0; i < frame->height / 2; i++) {
        memcpy(yuv_buffer.data() + y_size + i * (frame->width / 2),
               frame->data[1] + i * frame->linesize[1],
               frame->width / 2);
    }
    
    for (int i = 0; i < frame->height / 2; i++) {
        memcpy(yuv_buffer.data() + y_size + uv_size + i * (frame->width / 2),
               frame->data[2] + i * frame->linesize[2],
               frame->width / 2);
    }
    
    cv::Mat yuv_img(frame->height * 3 / 2, frame->width, CV_8UC1, yuv_buffer.data());
    cv::cvtColor(yuv_img, output, cv::COLOR_YUV2BGR_I420);
    
    return !output.empty();
}

cv::Mat VideoModule::getCurrentFrame(double targetTime) {
    std::lock_guard<std::mutex> queueLock(frameQueueMutex);
    
    // DEBUG: Print first 10 calls
    static int debugCount = 0;
    if (debugCount < 10) {
        std::cout << "getCurrentFrame called - targetTime: " << targetTime 
                  << ", queue size: " << frameQueue.size();
        if (!frameQueue.empty()) {
            std::cout << ", first frame ts: " << frameQueue.front().timestamp
                      << ", last frame ts: " << frameQueue.back().timestamp;
        }
        std::cout << std::endl;
    }
    
    // Drop old frames (more than 100ms behind)
    int droppedThisCall = 0;
    while (!frameQueue.empty() && frameQueue.front().timestamp < targetTime - 0.1) {
        frameQueue.pop_front();
        droppedFrames++;
        droppedThisCall++;
    }
    
    if (droppedThisCall > 0 && debugCount < 10) {
        std::cout << "Dropped " << droppedThisCall << " old frames" << std::endl;
    }
    
    // Check buffer health
    double fullness = static_cast<double>(frameQueue.size()) / maxQueueSize;
    if (fullness < bufferLowThreshold && !isBuffering) {
        isBuffering = true;
    } else if (isBuffering && fullness >= bufferFullnessThreshold) {
        isBuffering = false;
    }
    
    // If buffering, return last displayed frame
    if (isBuffering) {
        std::lock_guard<std::mutex> frameLock(currentFrameMutex);
        if (!currentFrame.empty()) {
            if (debugCount < 10) {
                std::cout << "Buffering active - returning last frame at ts: " 
                          << currentFrameTimestamp << std::endl;
            }
            debugCount++;
            return currentFrame.clone();
        }
        debugCount++;
        return cv::Mat();
    }
    
    // Find best frame for target time
    FrameBuffer* bestFrame = nullptr;
    double bestDiff = std::numeric_limits<double>::max();
    
    for (auto& fb : frameQueue) {
        if (!fb.valid) continue;
        
        double diff = std::abs(fb.timestamp - targetTime);
        if (diff < bestDiff) {
            bestFrame = &fb;
            bestDiff = diff;
        }
        
        // Stop searching if too far ahead
        if (fb.timestamp > targetTime + 0.05) break;
    }
    
    // Update current frame if found good match
    if (bestFrame && bestDiff < 0.1) {
        std::lock_guard<std::mutex> frameLock(currentFrameMutex);
        currentFrame = bestFrame->frame.clone();
        currentFrameTimestamp = bestFrame->timestamp;
        renderedFrames++;
        if (debugCount < 10) {
            std::cout << "Updated current frame - ts: " << currentFrameTimestamp 
                      << ", diff: " << bestDiff << std::endl;
        }
    }
    
    std::lock_guard<std::mutex> frameLock(currentFrameMutex);
    if (!currentFrame.empty()) {
        if (debugCount < 10) {
            std::cout << "Returning current frame - ts: " 
                      << currentFrameTimestamp << std::endl;
        }
        debugCount++;
        return currentFrame.clone();
    }
    
    if (debugCount < 10) {
        std::cout << "No frame available, returning empty" << std::endl;
    }
    debugCount++;
    return cv::Mat();
}


cv::Mat VideoModule::getCurrentFrameImmediate() {
    std::lock_guard<std::mutex> lock(currentFrameMutex);
    if (!currentFrame.empty()) {
        return currentFrame.clone();
    }
    return cv::Mat();
}

bool VideoModule::seek(double timeSeconds) {
    if (!isStreamLoaded() || timeSeconds < 0 || timeSeconds > totalDuration) {
        return false;
    }
    
    seekTargetTime = timeSeconds;
    seekRequested = true;
    return true;
}

void VideoModule::performSeek(double targetTime) {
    {
        std::lock_guard<std::mutex> lock(frameQueueMutex);
        frameQueue.clear();
    }
    
    int64_t ts = av_rescale_q(static_cast<int64_t>(targetTime * AV_TIME_BASE), 
                            AV_TIME_BASE_Q, 
                            formatContext->streams[streamIndex]->time_base);
    
    if (av_seek_frame(formatContext, streamIndex, ts, AVSEEK_FLAG_BACKWARD) >= 0) {
        avcodec_flush_buffers(codecContext);
        isBuffering = true;
    }
}

bool VideoModule::isInitialized() const { return true; }
bool VideoModule::isStreamLoaded() const { return codecContext != nullptr && formatContext != nullptr; }
bool VideoModule::getIsDecoding() const { return isDecoding.load(); }

VideoStats VideoModule::getStats() const {
    VideoStats stats;
    stats.decodedFrames = decodedFrames.load();
    stats.droppedFrames = droppedFrames.load();
    stats.renderedFrames = renderedFrames.load();
    
    {
        std::lock_guard<std::mutex> lock(frameQueueMutex);
        stats.queueSize = frameQueue.size();
    }
    
    stats.fps = fps;
    stats.currentTimestamp = currentFrameTimestamp;
    stats.hardwareAccelEnabled = (hwDeviceType != AV_HWDEVICE_TYPE_NONE);
    
    if (stats.hardwareAccelEnabled) {
        stats.hwAccelType = av_hwdevice_get_type_name(hwDeviceType);
    }
    
    return stats;
}

bool VideoModule::isReadyToPlay() const {
    std::lock_guard<std::mutex> lock(frameQueueMutex);
    return static_cast<double>(frameQueue.size()) / maxQueueSize >= bufferFullnessThreshold;
}

bool VideoModule::needsBuffering() const {
    return isBuffering.load();
}

void VideoModule::waitForBuffer() {
    while (isBuffering.load() && !shouldStop) {
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
}

void VideoModule::shutdown() {
    stopDecoding();
    
    if (codecContext) {
        avcodec_free_context(&codecContext);
    }
    
    if (hwDeviceCtx) {
        av_buffer_unref(&hwDeviceCtx);
        hwDeviceCtx = nullptr;
    }
    
    formatContext = nullptr;
    streamIndex = -1;
    hwDeviceType = AV_HWDEVICE_TYPE_NONE;
}