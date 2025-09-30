#include "video_module.h"
#include <algorithm>
#include <iostream>

// FrameBuffer implementation
FrameBuffer::FrameBuffer() : timestamp(0.0), valid(false) {}

FrameBuffer::FrameBuffer(FrameBuffer &&other) noexcept
    : frame(std::move(other.frame)), timestamp(other.timestamp),
      valid(other.valid) {
  other.valid = false;
}

FrameBuffer &FrameBuffer::operator=(FrameBuffer &&other) noexcept {
  if (this != &other) {
    frame = std::move(other.frame);
    timestamp = other.timestamp;
    valid = other.valid;
    other.valid = false;
  }
  return *this;
}

// VideoModule implementation
VideoModule::VideoModule()
    : formatContext(nullptr), codecContext(nullptr), streamIndex(-1),
      hwDeviceType(AV_HWDEVICE_TYPE_NONE), hwDeviceCtx(nullptr), videoWidth(0),
      videoHeight(0), fps(30.0), totalDuration(0.0), frameDuration(1.0 / 30.0),
      currentFrameTimestamp(0.0) {

  frameQueue =
      std::make_unique<LockFreeRingBuffer<FrameBuffer, MAX_FRAME_QUEUE_SIZE>>();
}

VideoModule::~VideoModule() { shutdown(); }

bool VideoModule::initialize(const VideoConfig &cfg) {
  std::cout << "Initializing VideoModule..." << std::endl;

  config = cfg;

  //if (config.enableHardwareAccel) {
  //  initializeHardwareAccel();
  //}

  std::cout << "VideoModule initialized successfully" << std::endl;
  return true;
}

bool VideoModule::initializeHardwareAccel() {
  const char *hw_types[] = {"vaapi", "nvdec", "dxva2", "videotoolbox"};

  for (const char *type_name : hw_types) {
    AVHWDeviceType type = av_hwdevice_find_type_by_name(type_name);
    if (type != AV_HWDEVICE_TYPE_NONE) {
      if (av_hwdevice_ctx_create(&hwDeviceCtx, type, nullptr, nullptr, 0) >=
          0) {
        hwDeviceType = type;
        std::cout << "Hardware acceleration enabled: " << type_name
                  << std::endl;
        return true;
      }
    }
  }

  std::cout << "Hardware acceleration not available, using software decoding"
            << std::endl;
  return false;
}

bool VideoModule::loadStream(AVFormatContext* ctx, int videoStreamIndex) {
    if (!ctx) {
        std::cerr << "VideoModule: Null format context" << std::endl;
        return false;
    }
    
    if (videoStreamIndex < 0 || videoStreamIndex >= static_cast<int>(ctx->nb_streams)) {
        std::cerr << "VideoModule: Invalid stream index " << videoStreamIndex 
                  << " (total streams: " << ctx->nb_streams << ")" << std::endl;
        return false;
    }
    
    AVCodecParameters* codecpar = ctx->streams[videoStreamIndex]->codecpar;
    if (codecpar->codec_type != AVMEDIA_TYPE_VIDEO) {
        std::cerr << "VideoModule: Stream " << videoStreamIndex 
                  << " is not a video stream" << std::endl;
        return false;
    }
    
    std::cout << "VideoModule::loadStream called with ctx=" << ctx 
              << ", streamIndex=" << videoStreamIndex << std::endl;

    formatContext = ctx;   // 🔑 Ensure assignment happens before setup
    streamIndex = videoStreamIndex;
    
    if (!setupDecoder()) {
        std::cerr << "VideoModule: Failed to setup video decoder" << std::endl;
        return false;
    }

    // Get video properties
    AVStream *videoStream = formatContext->streams[streamIndex];
    fps = av_q2d(videoStream->r_frame_rate);
    if (fps <= 0 || fps > 120)
        fps = 30.0; // Fallback
    frameDuration = 1.0 / fps;

    totalDuration = formatContext->duration / static_cast<double>(AV_TIME_BASE);

    // Get and scale dimensions
    int origWidth = codecContext->width;
    int origHeight = codecContext->height;

    if (origWidth > config.maxWidth || origHeight > config.maxHeight) {
        double scale = std::min(static_cast<double>(config.maxWidth) / origWidth,
                                static_cast<double>(config.maxHeight) / origHeight);
        videoWidth = static_cast<int>(origWidth * scale);
        videoHeight = static_cast<int>(origHeight * scale);
    } else {
        videoWidth = origWidth;
        videoHeight = origHeight;
    }

    // Clear statistics
    frameQueue->clear();
    decodedFrames = 0;
    droppedFrames = 0;
    renderedFrames = 0;

    std::cout << "VideoModule::loadStream complete. formatContext=" << formatContext 
              << ", streamIndex=" << streamIndex << std::endl;
    std::cout << "Resolution: " << videoWidth << "x" << videoHeight
              << " (original: " << origWidth << "x" << origHeight << ")" << std::endl;
    std::cout << "FPS: " << fps << ", Duration: " << totalDuration << "s" << std::endl;

    return true;
}


bool VideoModule::setupDecoder() {
    AVCodecParameters *codecParams =
        formatContext->streams[streamIndex]->codecpar;
    const AVCodec *codec = avcodec_find_decoder(codecParams->codec_id);

    if (!codec) {
        std::cerr << "Video codec not found" << std::endl;
        return false;
    }

    codecContext = avcodec_alloc_context3(codec);
    if (!codecContext) {
        std::cerr << "Failed to allocate video codec context" << std::endl;
        return false;
    }

    if (avcodec_parameters_to_context(codecContext, codecParams) < 0) {
        std::cerr << "Failed to copy video codec parameters" << std::endl;
        avcodec_free_context(&codecContext);
        return false;
    }

    // Try to set hardware acceleration
    if (hwDeviceCtx) {
        codecContext->hw_device_ctx = av_buffer_ref(hwDeviceCtx);
    }

    // --- New resilience settings ---
    av_opt_set(codecContext->priv_data, "threads", "auto", 0);
    av_opt_set(codecContext->priv_data, "thread_type", "frame", 0);

    codecContext->error_concealment = FF_EC_GUESS_MVS | FF_EC_DEBLOCK;
    codecContext->skip_loop_filter = AVDISCARD_DEFAULT;
    // -------------------------------

    if (avcodec_open2(codecContext, codec, nullptr) < 0) {
        std::cerr << "Failed to open video codec" << std::endl;
        avcodec_free_context(&codecContext);
        return false;
    }

    return true;
}


void VideoModule::startDecoding() {
    if (!isStreamLoaded()) {
        std::cerr << "Cannot start decoding: no stream loaded" << std::endl;
        return;
    }
    
    if (isDecoding) return;
    
    shouldStop = false;
    isDecoding = true;
    shouldRender = true;
    
    decodeThread = std::thread(&VideoModule::decodeThreadFunction, this);
    renderThread = std::thread(&VideoModule::renderThreadFunction, this);
    
    std::cout << "Video decoding started" << std::endl;
}

void VideoModule::stopDecoding() {
    if (!isDecoding) return;
    
    isDecoding = false;
    shouldRender = false;
    shouldStop = true;
    
    if (decodeThread.joinable()) {
        decodeThread.join();
    }
    
    if (renderThread.joinable()) {
        renderThread.join();
    }
    
    std::cout << "Video decoding stopped" << std::endl;
}

void VideoModule::renderThreadFunction() {
    auto lastFrameTime = std::chrono::steady_clock::now();
    const auto frameInterval = std::chrono::microseconds(static_cast<int64_t>(frameDuration * 1000000));
    
    while (shouldRender && !shouldStop) {
        auto currentTime = std::chrono::steady_clock::now();
        
        if (currentTime - lastFrameTime >= frameInterval) {
            // Update current frame from queue
            FrameBuffer fb;
            while (frameQueue->pop(fb)) {
                if (fb.valid && !fb.frame.empty()) {
                    std::lock_guard<std::mutex> lock(currentFrameMutex);
                    currentFrame = std::move(fb.frame);
                    currentFrameTimestamp = fb.timestamp;
                    renderedFrames++;
                    break; // Take first valid frame
                }
            }
            lastFrameTime = currentTime;
        }
        
        std::this_thread::sleep_for(std::chrono::microseconds(1000));
    }
}

void VideoModule::decodeThreadFunction() {
    std::cout << "DEBUG: Video decode thread started" << std::endl;
    std::cout << "DEBUG: formatContext=" << formatContext 
              << ", streamIndex=" << streamIndex 
              << ", shouldStop=" << shouldStop.load()
              << ", isDecoding=" << isDecoding.load() << std::endl;
    
    if (!formatContext) {
        std::cerr << "ERROR: formatContext is NULL in decode thread!" << std::endl;
        return;
    }
    
    if (streamIndex < 0) {
        std::cerr << "ERROR: invalid streamIndex in decode thread!" << std::endl;
        return;
    }

    AVPacket* packet = av_packet_alloc();
    AVFrame* frame = av_frame_alloc();
    AVFrame* hw_frame = nullptr;
    
    if (hwDeviceCtx) {
        hw_frame = av_frame_alloc();
    }
    
    int packets_read = 0;
    int frames_decoded = 0;
    
    while (!shouldStop && isDecoding) {
        if (seekRequested.load()) {
            performSeek(seekTargetTime.load());
            seekRequested = false;
        }
        
        if (frameQueue->size() < config.frameQueueSize * 0.8) {
            int read_result = av_read_frame(formatContext, packet);
            if (read_result >= 0) {
                packets_read++;
                
                if (packet->stream_index == streamIndex) {
                    std::cout << "DEBUG: Processing video packet " << packets_read << std::endl;
                    
                    if (avcodec_send_packet(codecContext, packet) >= 0) {
                        while (avcodec_receive_frame(codecContext, frame) >= 0) {
                            frames_decoded++;
                            std::cout << "DEBUG: Decoded video frame " << frames_decoded << std::endl;
                            
                            AVFrame* decode_frame = frame;
                            
                            // Handle hardware decoding
                            if (hwDeviceCtx && hw_frame) {
                                if (avcodec_receive_frame(codecContext, hw_frame) >= 0) {
                                    if (av_hwframe_transfer_data(frame, hw_frame, 0) >= 0) {
                                        decode_frame = frame;
                                    }
                                }
                            }
                            
                            if (decode_frame->data[0]) {
                                FrameBuffer fb;
                                if (convertAVFrameToMat(decode_frame, fb.frame)) {
                                    AVStream* stream = formatContext->streams[streamIndex];
                                    if (decode_frame->pts != AV_NOPTS_VALUE) {
                                        fb.timestamp = decode_frame->pts * av_q2d(stream->time_base);
                                    } else {
                                        fb.timestamp = frames_decoded / fps;
                                    }
                                    fb.valid = true;
                                    
                                    if (!frameQueue->push(std::move(fb))) {
                                        droppedFrames++;
                                        std::cout << "DEBUG: Frame queue full, dropped frame" << std::endl;
                                    } else {
                                        decodedFrames++;
                                        std::cout << "DEBUG: Added frame to queue, queue size: " << frameQueue->size() << std::endl;
                                    }
                                } else {
                                    std::cout << "DEBUG: convertAVFrameToMat failed" << std::endl;
                                }
                            } else {
                                std::cout << "DEBUG: No frame data" << std::endl;
                            }
                        }
                    } else {
                        std::cout << "DEBUG: avcodec_send_packet failed" << std::endl;
                    }
                }
                av_packet_unref(packet);
            } else {
                std::cout << "DEBUG: av_read_frame failed or EOF: " << read_result << std::endl;
                break;
            }
        } else {
            std::this_thread::sleep_for(std::chrono::microseconds(1000));
        }
    }
    
    std::cout << "DEBUG: Video decode thread ending. Packets: " << packets_read 
              << ", Frames: " << frames_decoded << std::endl;
    
    av_frame_free(&frame);
    if (hw_frame) av_frame_free(&hw_frame);
    av_packet_free(&packet);
}



bool VideoModule::convertAVFrameToMat(AVFrame* frame, cv::Mat& output) {
    if (!frame || !frame->data[0] || frame->format != AV_PIX_FMT_YUV420P) {
        return false;
    }
    
    // Create a continuous buffer in YUV420p format for OpenCV
    int y_size = frame->width * frame->height;
    int uv_size = (frame->width / 2) * (frame->height / 2);
    
    std::vector<uint8_t> yuv_buffer(y_size + 2 * uv_size);
    
    // Copy Y plane
    for (int i = 0; i < frame->height; i++) {
        memcpy(yuv_buffer.data() + i * frame->width,
               frame->data[0] + i * frame->linesize[0],
               frame->width);
    }
    
    // Copy U plane
    for (int i = 0; i < frame->height / 2; i++) {
        memcpy(yuv_buffer.data() + y_size + i * (frame->width / 2),
               frame->data[1] + i * frame->linesize[1],
               frame->width / 2);
    }
    
    // Copy V plane
    for (int i = 0; i < frame->height / 2; i++) {
        memcpy(yuv_buffer.data() + y_size + uv_size + i * (frame->width / 2),
               frame->data[2] + i * frame->linesize[2],
               frame->width / 2);
    }
    
    // Create Mat from the buffer
    cv::Mat yuv_img(frame->height * 3 / 2, frame->width, CV_8UC1, yuv_buffer.data());
    
    // Convert YUV420p to BGR
    cv::Mat bgr_img;
    cv::cvtColor(yuv_img, bgr_img, cv::COLOR_YUV2BGR_I420);
    
    // Resize if needed
    if (bgr_img.cols != videoWidth || bgr_img.rows != videoHeight) {
        cv::resize(bgr_img, output, cv::Size(videoWidth, videoHeight));
    } else {
        output = bgr_img.clone();
    }
    
    return !output.empty();
}

cv::Mat VideoModule::getCurrentFrame(double targetTime) {
    std::cout << "DEBUG getCurrentFrame: targetTime=" << targetTime << std::endl;
    updateCurrentFrame(targetTime);
    
    std::lock_guard<std::mutex> lock(currentFrameMutex);
    if (!currentFrame.empty()) {
        renderedFrames++;
        return currentFrame.clone();
    }
    
    std::cout << "DEBUG: getCurrentFrame returning empty" << std::endl;
    return cv::Mat();
}

cv::Mat VideoModule::getCurrentFrameImmediate() {
    std::lock_guard<std::mutex> lock(currentFrameMutex);
    if (!currentFrame.empty()) {
        return currentFrame.clone();
    }
    return cv::Mat();
}

void VideoModule::updateCurrentFrame(double targetTime, double tolerance) {
    std::cout << "DEBUG updateCurrentFrame called, targetTime=" << targetTime 
              << ", queue size=" << frameQueue->size() << std::endl;
    
    FrameBuffer fb;
    FrameBuffer lastValidFrame;
    bool hasFrame = false;
    
    while (frameQueue->pop(fb)) {
        std::cout << "DEBUG: Popped frame, valid=" << fb.valid 
                  << ", timestamp=" << fb.timestamp << std::endl;
        if (fb.valid) {
            lastValidFrame = std::move(fb);
            hasFrame = true;
        }
    }
    
    if (hasFrame) {
        std::lock_guard<std::mutex> lock(currentFrameMutex);
        currentFrame = std::move(lastValidFrame.frame);
        currentFrameTimestamp = lastValidFrame.timestamp;
        std::cout << "DEBUG: Updated current frame, size=" << currentFrame.cols 
                  << "x" << currentFrame.rows << std::endl;
    } else {
        std::cout << "DEBUG: No valid frame found in queue" << std::endl;
    }
}

bool VideoModule::seek(double timeSeconds) {
  if (!isStreamLoaded() || timeSeconds < 0 || timeSeconds > totalDuration) {
    return false;
  }

  seekTargetTime = timeSeconds;
  seekRequested = true;

  std::cout << "Video seek requested to: " << timeSeconds << "s" << std::endl;
  return true;
}

void VideoModule::performSeek(double targetTime) {
  std::cout << "Performing video seek to: " << targetTime << "s" << std::endl;

  // Clear frame queue
  frameQueue->clear();

  // Seek in format context
  int64_t ts = av_rescale_q(static_cast<int64_t>(targetTime * AV_TIME_BASE),
                            AV_TIME_BASE_Q,
                            formatContext->streams[streamIndex]->time_base);

  if (av_seek_frame(formatContext, streamIndex, ts, AVSEEK_FLAG_BACKWARD) >=
      0) {
    // Flush codec buffers
    avcodec_flush_buffers(codecContext);
    std::cout << "Video seek completed" << std::endl;
  } else {
    std::cerr << "Video seek failed" << std::endl;
  }
}

bool VideoModule::isInitialized() const {
  return true; // VideoModule doesn't need complex initialization
}

bool VideoModule::isStreamLoaded() const {
  return codecContext != nullptr && formatContext != nullptr;
}

bool VideoModule::getIsDecoding() const { return isDecoding.load(); }

VideoStats VideoModule::getStats() const {
  VideoStats stats;
  stats.decodedFrames = decodedFrames.load();
  stats.droppedFrames = droppedFrames.load();
  stats.renderedFrames = renderedFrames.load();
  stats.queueSize = frameQueue->size();
  stats.fps = fps;
  stats.currentTimestamp = currentFrameTimestamp;
  stats.hardwareAccelEnabled = (hwDeviceType != AV_HWDEVICE_TYPE_NONE);

  if (stats.hardwareAccelEnabled) {
    stats.hwAccelType = av_hwdevice_get_type_name(hwDeviceType);
  }

  return stats;
}

void VideoModule::shutdown() {
  std::cout << "Shutting down VideoModule..." << std::endl;

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