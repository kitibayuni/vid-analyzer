#include "video_module.h"
#include <iostream>
#include <algorithm>
#include <limits>
#include <thread>
#include <chrono>
#include <condition_variable>

extern "C" {
#include <libswscale/swscale.h>
}

// FrameBuffer implementation moved to header

VideoModule::VideoModule()
    : formatContext(nullptr), codecContext(nullptr), streamIndex(-1),
      hwDeviceType(AV_HWDEVICE_TYPE_NONE), hwDeviceCtx(nullptr), hwPixelFormat(AV_PIX_FMT_NONE),
      videoWidth(0), videoHeight(0), fps(30.0), totalDuration(0.0), frameDuration(1.0/30.0),
      minPacketQueueSize(50), currentFrameTimestamp(0.0),
      actualThreadCount(1), useHardwareDecoding(false),
      bufferFullnessThreshold(0.3), bufferLowThreshold(0.15), decodeAheadTime(2.0) {

    // Initialize queues with optimized sizes
    packetQueue = std::make_unique<ThreadSafeQueue<PacketHolder>>(150, shouldStop);
    frameQueue = std::make_unique<ThreadSafeQueue<FrameBuffer>>(120, shouldStop);
}

VideoModule::~VideoModule() {
    shutdown();
}

bool VideoModule::initialize(const VideoConfig& cfg) {
    std::cout << "Initializing VideoModule..." << std::endl;
    config = cfg;

    if (config.enableHardwareAccel) {
        initializeHardwareAccel();
    }

    std::cout << "VideoModule initialized successfully" << std::endl;
    return true;
}

bool VideoModule::initializeHardwareAccel() {
    // Priority order: NVDEC (NVIDIA) > VAAPI (Intel/AMD on Linux) > DXVA2 (Windows) > VideoToolbox (macOS)
    const char* hw_types[] = {"cuda", "vaapi", "dxva2", "videotoolbox", "qsv"};

    for (const char* type_name : hw_types) {
        AVHWDeviceType type = av_hwdevice_find_type_by_name(type_name);
        if (type != AV_HWDEVICE_TYPE_NONE) {
            std::cout << "Attempting hardware acceleration: " << type_name << "..." << std::endl;
            if (av_hwdevice_ctx_create(&hwDeviceCtx, type, nullptr, nullptr, 0) >= 0) {
                hwDeviceType = type;
                std::cout << "✓ Hardware acceleration enabled: " << type_name << std::endl;
                return true;
            } else {
                std::cout << "✗ Failed to initialize " << type_name << std::endl;
            }
        }
    }

    std::cout << "Hardware acceleration not available, using software decoding" << std::endl;
    return false;
}

int VideoModule::detectOptimalThreadCount() {
    int hwConcurrency = std::thread::hardware_concurrency();

    if (hwConcurrency == 0) {
        std::cout << "Unable to detect CPU cores, using 2 threads" << std::endl;
        return 2;
    }

    // Use 50-75% of available cores for moderate usage
    // Use at least 2 threads, but cap at 8 for diminishing returns
    int optimalThreads = std::max(2, std::min(8, static_cast<int>(hwConcurrency * 0.6)));

    std::cout << "Detected " << hwConcurrency << " CPU cores, using "
              << optimalThreads << " decoder threads" << std::endl;

    return optimalThreads;
}

void VideoModule::calculateDynamicBufferParams() {
    // Calculate decode-ahead time based on FPS
    // Higher FPS = shorter buffer time (more frames per second)
    // Lower FPS = longer buffer time (fewer frames per second)

    if (fps >= 60.0) {
        // High FPS (60+): 1.5 seconds ahead
        decodeAheadTime = 1.5;
        bufferFullnessThreshold = 0.25;  // 25%
        bufferLowThreshold = 0.12;       // 12%
    } else if (fps >= 30.0) {
        // Normal FPS (30-59): 2.0 seconds ahead
        decodeAheadTime = 2.0;
        bufferFullnessThreshold = 0.30;  // 30%
        bufferLowThreshold = 0.15;       // 15%
    } else if (fps >= 24.0) {
        // Cinema FPS (24-29): 2.5 seconds ahead
        decodeAheadTime = 2.5;
        bufferFullnessThreshold = 0.35;  // 35%
        bufferLowThreshold = 0.18;       // 18%
    } else {
        // Low FPS (<24): 3.0 seconds ahead
        decodeAheadTime = 3.0;
        bufferFullnessThreshold = 0.40;  // 40%
        bufferLowThreshold = 0.20;       // 20%
    }

    std::cout << "Dynamic buffer params for " << fps << " FPS:" << std::endl;
    std::cout << "  - Decode ahead: " << decodeAheadTime << "s" << std::endl;
    std::cout << "  - Buffer fullness threshold: " << (bufferFullnessThreshold * 100) << "%" << std::endl;
    std::cout << "  - Buffer low threshold: " << (bufferLowThreshold * 100) << "%" << std::endl;
}

AVFrame* VideoModule::getHWFrame() {
    std::lock_guard<std::mutex> lock(hwFramePoolMutex);
    if (!hwFramePool.empty()) {
        AVFrame* frame = hwFramePool.back();
        hwFramePool.pop_back();
        av_frame_unref(frame);  // Clear previous data
        return frame;
    }
    return av_frame_alloc();  // Create new if pool empty
}

void VideoModule::returnHWFrame(AVFrame* frame) {
    if (!frame) return;

    std::lock_guard<std::mutex> lock(hwFramePoolMutex);
    if (hwFramePool.size() < MAX_HW_FRAME_POOL_SIZE) {
        av_frame_unref(frame);  // Clear data but keep allocation
        hwFramePool.push_back(frame);
    } else {
        av_frame_free(&frame);  // Pool full, actually free
    }
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

    // Calculate dynamic buffer parameters based on FPS
    calculateDynamicBufferParams();

    // Clear frame queue
    frameQueue->clear();
    decodedFrames = 0;
    droppedFrames = 0;
    renderedFrames = 0;

    std::cout << "Video loaded: " << videoWidth << "x" << videoHeight
              << " @ " << fps << "fps" << std::endl;

    // Start verification thread
    isVerified = false;
    isVerifying = true;
    verificationErrorMsg.clear();
    verificationThread = std::thread(&VideoModule::verificationThreadFunction, this);

    return true;
}

bool VideoModule::setupDecoder() {
    AVCodecParameters* codecParams = formatContext->streams[streamIndex]->codecpar;
    const AVCodec* codec = nullptr;

    // Try to find hardware decoder first if HW accel is available
    if (hwDeviceCtx) {
        // Map codec to hardware decoder name
        std::string hwCodecName;
        const char* codecName = avcodec_get_name(codecParams->codec_id);

        if (hwDeviceType == AV_HWDEVICE_TYPE_CUDA) {
            if (codecParams->codec_id == AV_CODEC_ID_H264) hwCodecName = "h264_cuvid";
            else if (codecParams->codec_id == AV_CODEC_ID_HEVC) hwCodecName = "hevc_cuvid";
            else if (codecParams->codec_id == AV_CODEC_ID_VP9) hwCodecName = "vp9_cuvid";
            else if (codecParams->codec_id == AV_CODEC_ID_MPEG2VIDEO) hwCodecName = "mpeg2_cuvid";
        } else if (hwDeviceType == AV_HWDEVICE_TYPE_VAAPI) {
            // VAAPI uses standard decoders with get_format callback
        }

        if (!hwCodecName.empty()) {
            codec = avcodec_find_decoder_by_name(hwCodecName.c_str());
            if (codec) {
                std::cout << "Using hardware decoder: " << hwCodecName << std::endl;
            }
        }
    }

    // Fallback to software decoder
    if (!codec) {
        codec = avcodec_find_decoder(codecParams->codec_id);
        if (hwDeviceCtx) {
            std::cout << "Hardware decoder not found, falling back to software with HW transfer" << std::endl;
        }
    }

    if (!codec) return false;

    codecContext = avcodec_alloc_context3(codec);
    if (!codecContext) return false;

    if (avcodec_parameters_to_context(codecContext, codecParams) < 0) {
        avcodec_free_context(&codecContext);
        return false;
    }

    // Set hardware device context
    if (hwDeviceCtx) {
        codecContext->hw_device_ctx = av_buffer_ref(hwDeviceCtx);
        useHardwareDecoding = true;

        // For VAAPI/DXVA2, we need to set get_format callback to select HW pixel format
        if (hwDeviceType == AV_HWDEVICE_TYPE_VAAPI) {
            codecContext->get_format = [](AVCodecContext *ctx, const enum AVPixelFormat *pix_fmts) -> enum AVPixelFormat {
                const enum AVPixelFormat *p;
                for (p = pix_fmts; *p != AV_PIX_FMT_NONE; p++) {
                    if (*p == AV_PIX_FMT_VAAPI)
                        return *p;
                }
                return AV_PIX_FMT_NONE;
            };
        }

        std::cout << "Hardware decoding context set for GPU offload" << std::endl;
    } else {
        useHardwareDecoding = false;
    }

    // Configure multi-threading (for software decoding or hybrid)
    if (config.threadCount == 0) {
        actualThreadCount = detectOptimalThreadCount();
    } else if (config.threadCount > 0) {
        actualThreadCount = config.threadCount;
    } else {
        actualThreadCount = 1;
    }

    codecContext->thread_count = actualThreadCount;
    codecContext->thread_type = FF_THREAD_FRAME | FF_THREAD_SLICE;

    std::cout << "Configuring decoder with " << actualThreadCount << " threads" << std::endl;

    codecContext->error_concealment = FF_EC_GUESS_MVS | FF_EC_DEBLOCK;

    if (avcodec_open2(codecContext, codec, nullptr) < 0) {
        avcodec_free_context(&codecContext);
        return false;
    }

    // Report actual thread count used by codec
    std::cout << "Decoder initialized with " << codecContext->thread_count
              << " threads (type: ";
    if (codecContext->active_thread_type & FF_THREAD_FRAME) std::cout << "FRAME ";
    if (codecContext->active_thread_type & FF_THREAD_SLICE) std::cout << "SLICE";
    std::cout << ")" << std::endl;

    return true;
}

void VideoModule::startDecoding() {
    if (!isStreamLoaded() || isDemuxing || isDecoding) return;

    // Wait for verification to complete before starting playback
    std::cout << "Waiting for video integrity verification..." << std::endl;
    waitForVerification();

    if (!isVerified.load()) {
        std::cerr << "Cannot start decoding: Video verification failed - "
                  << verificationErrorMsg << std::endl;
        return;
    }

    std::cout << "Video verification passed, starting decoding..." << std::endl;

    shouldStop = false;
    isDemuxing = true;
    isDecoding = true;
    isBuffering = true;

    // Start 2-stage pipeline (demux + decode/convert)
    demuxThread = std::thread(&VideoModule::demuxThreadFunction, this);
    decodeThread = std::thread(&VideoModule::decodeThreadFunction, this);

    std::cout << "Starting optimized 2-stage pipeline:" << std::endl;
    std::cout << "  Stage 1 [DEMUX]: Reading packets from file (150 packet buffer)" << std::endl;
    std::cout << "  Stage 2 [DECODE+CONVERT]: " << (useHardwareDecoding ? "GPU" : "CPU")
              << " decoding with " << actualThreadCount << " threads + immediate conversion (120 frame buffer)" << std::endl;
    std::cout << "  Stage 3 [RENDER]: Video rendering on main thread" << std::endl;
    std::cout << "Preloading video buffer..." << std::endl;

    // Use condition variable to wait for buffer instead of sleep polling
    const auto timeout = std::chrono::seconds(5);

    // Calculate target frames for initial buffer (1 second worth of frames based on detected fps)
    size_t targetFrames = static_cast<size_t>(fps);
    const size_t maxFrames = 120;
    if (targetFrames > maxFrames) targetFrames = maxFrames / 2;

    std::unique_lock<std::mutex> lock(bufferingMutex);
    const auto startTime = std::chrono::steady_clock::now();

    bool ready = bufferingCV.wait_for(lock, timeout, [&] {
        const size_t queueSize = frameQueue->size();
        // Exit conditions:
        // 1. Reached target frame count
        // 2. Reached buffer fullness threshold
        // 3. Decode thread stopped (EOF or error)
        // 4. Shutdown requested
        return queueSize >= targetFrames ||
               static_cast<double>(queueSize) / maxFrames >= bufferFullnessThreshold ||
               !isDecoding.load() ||
               shouldStop.load();
    });

    const size_t finalSize = frameQueue->size();
    isBuffering = false;

    std::cout << "Buffer ready: " << finalSize << " frames";
    if (!ready) {
        const auto elapsed = std::chrono::steady_clock::now() - startTime;
        std::cout << " (timeout after " << std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count() << "ms)";
    }
    std::cout << std::endl;

    std::cout << "Video pipeline started successfully" << std::endl;
}

void VideoModule::stopDecoding() {
    if (!isDecoding && !isDemuxing) return;

    std::cout << "Stopping video pipeline..." << std::endl;

    isDecoding = false;
    isDemuxing = false;
    shouldStop = true;

    // Wake up all waiting threads
    packetQueue->notifyAll();
    frameQueue->notifyAll();

    if (demuxThread.joinable()) {
        demuxThread.join();
        std::cout << "  - Demux thread stopped" << std::endl;
    }

    if (decodeThread.joinable()) {
        decodeThread.join();
        std::cout << "  - Decode thread stopped" << std::endl;
    }

    // Clear all queues (RAII handles cleanup)
    packetQueue->clear();
    frameQueue->clear();

    std::cout << "Video pipeline stopped" << std::endl;
}

bool VideoModule::isVerificationComplete() const {
    return !isVerifying.load();
}

void VideoModule::waitForVerification() {
    std::unique_lock<std::mutex> lock(verificationMutex);
    verificationCV.wait(lock, [this] {
        return !isVerifying.load();
    });
}

void VideoModule::demuxThreadFunction() {
    std::cout << "[DEMUX] Thread started" << std::endl;

    size_t packetsRead = 0;
    const auto startTime = std::chrono::steady_clock::now();

    while (!shouldStop && isDemuxing) {
        // Cache atomic loads for seek operation
        if (seekRequested.load()) {
            const double targetTime = seekTargetTime.load();
            seekRequested = false;
            performSeek(targetTime);
            continue;
        }

        PacketHolder holder;
        if (av_read_frame(formatContext, holder.packet) >= 0) {
            if (holder.packet->stream_index == streamIndex) {
                if (packetQueue->push(std::move(holder), std::chrono::milliseconds(50))) {
                    packetsRead++;
                }
            }
        } else {
            // EOF
            isDemuxing = false;
            packetQueue->notifyAll();
            break;
        }
    }

    const auto elapsed = std::chrono::steady_clock::now() - startTime;
    const auto elapsedMs = std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count();
    std::cout << "[DEMUX] Finished - " << packetsRead << " packets in "
              << elapsedMs << "ms (" << (packetsRead * 1000.0 / std::max(elapsedMs, 1L)) << " pkt/s)" << std::endl;
}

void VideoModule::decodeThreadFunction() {
    std::cout << "[DECODE] Thread started (" << (useHardwareDecoding ? "GPU" : "CPU")
              << " mode, " << actualThreadCount << " threads)" << std::endl;

    size_t framesDecoded = 0;
    const auto startTime = std::chrono::steady_clock::now();
    AVStream* stream = formatContext->streams[streamIndex];

    // Calculate target queue size based on FPS (decode ahead time)
    const size_t targetQueueSize = static_cast<size_t>(fps * decodeAheadTime);
    const size_t minQueueSize = static_cast<size_t>(fps * 0.5);  // Minimum 0.5s buffer

    std::cout << "[DECODE] Target queue size: " << targetQueueSize
              << " frames (" << decodeAheadTime << "s @ " << fps << " fps)" << std::endl;

    // Wait for minimum packets for better batching
    packetQueue->waitForMinSize(minPacketQueueSize, std::chrono::milliseconds(500));

    double lastDecodedTimestamp = 0.0;

    while (!shouldStop && isDecoding) {
        // Throttle decoding based on playback position and queue size
        const size_t queueSize = frameQueue->size();
        const double currentPlaybackTime = lastRequestedTime.load();
        const bool isBufferingNow = isBuffering.load();

        // During normal playback, throttle if:
        // 1. Queue is sufficiently full (has minimum buffer)
        // 2. We've decoded far enough ahead of current playback position
        if (!isBufferingNow && queueSize >= minQueueSize) {
            const double decodeAhead = lastDecodedTimestamp - currentPlaybackTime;
            if (decodeAhead >= decodeAheadTime) {
                std::this_thread::sleep_for(std::chrono::milliseconds(10));
                continue;
            }
        }

        // Also throttle if queue is at max capacity
        if (!isBufferingNow && queueSize >= targetQueueSize) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
            continue;
        }

        PacketHolder packetHolder;
        if (!packetQueue->pop(packetHolder, std::chrono::milliseconds(100))) {
            if (!isDemuxing && !shouldStop) {
                // Flush decoder at EOF
                avcodec_send_packet(codecContext, nullptr);
                isDecoding = false;
                frameQueue->notifyAll();
                break;
            }
            continue;
        }

        // Decode packet (happens on GPU if HW accel enabled)
        if (avcodec_send_packet(codecContext, packetHolder.packet) >= 0) {
            while (!shouldStop) {
                FrameHolder frameHolder;
                const int ret = avcodec_receive_frame(codecContext, frameHolder.frame);

                if (ret == 0 && frameHolder.frame->data[0]) {
                    // Calculate timestamp
                    const double timestamp = (frameHolder.frame->pts != AV_NOPTS_VALUE)
                        ? frameHolder.frame->pts * av_q2d(stream->time_base)
                        : framesDecoded / fps;

                    lastDecodedTimestamp = timestamp;

                    // CRITICAL FIX: Convert immediately to prevent corruption
                    // The separate conversion thread causes buffer reuse issues
                    // Convert now while frame data is still valid
                    cv::Mat convertedFrame;
                    if (convertAVFrameToMat(frameHolder.frame, convertedFrame)) {
                        // Use shared_ptr to avoid expensive cloning later
                        auto sharedFrame = std::make_shared<cv::Mat>(std::move(convertedFrame));
                        FrameBuffer fb(sharedFrame, timestamp);
                        if (frameQueue->push(std::move(fb), std::chrono::milliseconds(50))) {
                            framesDecoded++;
                            decodedFrames++;
                            // Notify waiting threads that buffer state changed
                            bufferingCV.notify_all();
                        }
                    }
                    // frameHolder freed here, buffer returned to decoder pool
                } else if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                    break;
                }
            }
        }
    }

    const auto elapsed = std::chrono::steady_clock::now() - startTime;
    const auto elapsedMs = std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count();
    std::cout << "[DECODE] Finished - " << framesDecoded << " frames in "
              << elapsedMs << "ms (" << (framesDecoded * 1000.0 / std::max(elapsedMs, 1L)) << " fps)" << std::endl;
}

void VideoModule::verificationThreadFunction() {
    std::cout << "[VERIFY] Starting video integrity verification..." << std::endl;
    const auto startTime = std::chrono::steady_clock::now();

    bool success = verifyStreamIntegrity();

    const auto elapsed = std::chrono::steady_clock::now() - startTime;
    const auto elapsedMs = std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count();

    if (success) {
        std::cout << "[VERIFY] ✓ Video integrity verified in " << elapsedMs << "ms" << std::endl;
        isVerified = true;
    } else {
        std::cerr << "[VERIFY] ✗ Verification failed: " << verificationErrorMsg
                  << " (" << elapsedMs << "ms)" << std::endl;
        isVerified = false;
    }

    isVerifying = false;
    verificationCV.notify_all();
}

bool VideoModule::verifyStreamIntegrity() {
    // 1. Verify codec context is valid
    if (!codecContext || !formatContext) {
        verificationErrorMsg = "Invalid codec or format context";
        return false;
    }

    // 2. Verify stream parameters
    AVStream* stream = formatContext->streams[streamIndex];
    if (!stream) {
        verificationErrorMsg = "Invalid stream";
        return false;
    }

    // 3. Verify codec parameters
    if (codecContext->width <= 0 || codecContext->height <= 0) {
        verificationErrorMsg = "Invalid video dimensions";
        return false;
    }

    if (codecContext->width > 16384 || codecContext->height > 16384) {
        verificationErrorMsg = "Video dimensions exceed maximum (16384x16384)";
        return false;
    }

    // 4. Verify FPS is reasonable
    if (fps <= 0 || fps > 240) {
        verificationErrorMsg = "Invalid or unsupported frame rate: " + std::to_string(fps);
        return false;
    }

    // 5. Verify duration is positive
    if (totalDuration <= 0) {
        verificationErrorMsg = "Invalid video duration";
        return false;
    }

    // 6. Test decode a few frames to ensure codec works properly
    std::cout << "[VERIFY] Testing frame decoding..." << std::endl;

    // Save current position
    int64_t currentPos = avio_tell(formatContext->pb);

    // Seek to beginning
    av_seek_frame(formatContext, streamIndex, 0, AVSEEK_FLAG_BACKWARD);
    avcodec_flush_buffers(codecContext);

    int framesDecoded = 0;
    int packetsRead = 0;
    const int targetTestFrames = std::min(10, static_cast<int>(fps)); // Test 10 frames or 1 second
    const int maxPacketsToRead = 100; // Safety limit
    double lastTimestamp = -1.0;
    bool timestampIncreasing = true;

    PacketHolder testPacket;
    while (packetsRead < maxPacketsToRead && framesDecoded < targetTestFrames) {
        int readResult = av_read_frame(formatContext, testPacket.packet);

        if (readResult < 0) {
            if (readResult == AVERROR_EOF) {
                // Flush decoder
                avcodec_send_packet(codecContext, nullptr);
            } else {
                verificationErrorMsg = "Error reading packets during verification";
                return false;
            }
        }

        if (testPacket.packet->stream_index == streamIndex) {
            packetsRead++;

            if (avcodec_send_packet(codecContext, testPacket.packet) >= 0) {
                FrameHolder testFrame;
                while (avcodec_receive_frame(codecContext, testFrame.frame) == 0) {
                    // Verify frame has valid data
                    if (!testFrame.frame->data[0]) {
                        verificationErrorMsg = "Decoded frame has no data";
                        return false;
                    }

                    // Verify frame dimensions match
                    if (testFrame.frame->width != codecContext->width ||
                        testFrame.frame->height != codecContext->height) {
                        verificationErrorMsg = "Frame dimensions mismatch";
                        return false;
                    }

                    // Verify timestamp is valid and increasing
                    if (testFrame.frame->pts != AV_NOPTS_VALUE) {
                        double timestamp = testFrame.frame->pts * av_q2d(stream->time_base);
                        if (lastTimestamp >= 0 && timestamp < lastTimestamp) {
                            timestampIncreasing = false;
                        }
                        lastTimestamp = timestamp;
                    }

                    // Test conversion to Mat
                    cv::Mat testMat;
                    if (!convertAVFrameToMat(testFrame.frame, testMat)) {
                        verificationErrorMsg = "Failed to convert frame to Mat";
                        return false;
                    }

                    if (testMat.empty() || testMat.cols != codecContext->width ||
                        testMat.rows != codecContext->height) {
                        verificationErrorMsg = "Converted frame has invalid dimensions";
                        return false;
                    }

                    framesDecoded++;
                    if (framesDecoded >= targetTestFrames) break;
                }
            }
        }

        av_packet_unref(testPacket.packet);
    }

    if (framesDecoded == 0) {
        verificationErrorMsg = "Could not decode any test frames";
        return false;
    }

    if (!timestampIncreasing) {
        std::cout << "[VERIFY] Warning: Non-monotonic timestamps detected" << std::endl;
    }

    std::cout << "[VERIFY] Successfully decoded " << framesDecoded << " test frames" << std::endl;

    // Restore stream position and flush buffers
    avio_seek(formatContext->pb, currentPos, SEEK_SET);
    av_seek_frame(formatContext, streamIndex, 0, AVSEEK_FLAG_BACKWARD);
    avcodec_flush_buffers(codecContext);

    return true;
}

bool VideoModule::convertAVFrameToMat(AVFrame* frame, cv::Mat& output) {
    if (!frame || !frame->data[0]) {
        return false;
    }

    AVFrame* swFrame = nullptr;

    // Check if frame is in hardware format (needs transfer to CPU memory)
    if (frame->format == AV_PIX_FMT_CUDA ||
        frame->format == AV_PIX_FMT_VAAPI ||
        frame->format == AV_PIX_FMT_DXVA2_VLD ||
        frame->format == AV_PIX_FMT_VIDEOTOOLBOX ||
        frame->format == AV_PIX_FMT_QSV) {

        // Transfer from GPU to CPU using pooled frame
        swFrame = getHWFrame();
        if (!swFrame) return false;

        if (av_hwframe_transfer_data(swFrame, frame, 0) < 0) {
            returnHWFrame(swFrame);
            return false;
        }

        // Copy metadata
        av_frame_copy_props(swFrame, frame);
        frame = swFrame;
    }

    // Use swscale for all formats (SIMD-optimized, faster than manual memcpy)
    // Create cached sws context for better performance
    struct SwsContext* swsCtx = sws_getContext(
        frame->width, frame->height, static_cast<AVPixelFormat>(frame->format),
        frame->width, frame->height, AV_PIX_FMT_BGR24,
        SWS_FAST_BILINEAR,  // Faster than SWS_BILINEAR, good quality
        nullptr, nullptr, nullptr
    );

    bool success = false;
    if (swsCtx) {
        output = cv::Mat(frame->height, frame->width, CV_8UC3);
        uint8_t* dest[1] = { output.data };
        int destLinesize[1] = { static_cast<int>(output.step[0]) };

        sws_scale(swsCtx, frame->data, frame->linesize, 0, frame->height,
                 dest, destLinesize);

        sws_freeContext(swsCtx);
        success = !output.empty();
    }

    // Return pooled frame instead of freeing
    if (swFrame) {
        returnHWFrame(swFrame);
    }

    return success;
}

cv::Mat VideoModule::getCurrentFrame(double targetTime) {
    // Update playback position for decode throttling
    lastRequestedTime.store(targetTime);

    // Drop old frames (more than 200ms behind target time)
    // Keep at least 1 frame to ensure we always have something to display
    int dropped = frameQueue->dropFrontWhile(
        [targetTime](const FrameBuffer& fb) {
            return fb.timestamp < targetTime - 0.2;
        },
        1  // minimum frames to keep
    );

    if (dropped > 0) {
        droppedFrames += dropped;
    }

    // Check buffer health - cache atomic load
    const size_t queueSize = frameQueue->size();
    const size_t maxFrames = 120;
    const double fullness = static_cast<double>(queueSize) / maxFrames;
    const bool currentlyBuffering = isBuffering.load();

    if (fullness < bufferLowThreshold && !currentlyBuffering) {
        isBuffering = true;
    } else if (currentlyBuffering && fullness >= bufferFullnessThreshold) {
        isBuffering = false;
        bufferingCV.notify_all();  // Notify waiters that buffering is complete
    }

    // Single mutex lock for entire update/return operation
    std::lock_guard<std::mutex> frameLock(currentFrameMutex);

    // If buffering, return cached frame (no clone needed with shared_ptr!)
    if (currentlyBuffering) {
        return (currentFrame && !currentFrame->empty()) ? *currentFrame : cv::Mat();
    }

    // Check if we should advance to the next frame
    // Only pop a new frame if:
    // 1. Current frame is significantly behind target time (more than half a frame duration)
    // 2. Or we don't have a current frame yet
    const double frameTimeTolerance = frameDuration * 0.5;
    const bool shouldAdvance = (!currentFrame || currentFrame->empty() ||
                               (targetTime - currentFrameTimestamp) > frameTimeTolerance);

    if (shouldAdvance) {
        FrameBuffer nextFrame;
        // Use non-blocking pop to check if next frame is available
        if (frameQueue->pop(nextFrame, std::chrono::milliseconds(0))) {
            if (nextFrame.valid && nextFrame.frame) {
                currentFrame = nextFrame.frame;  // Share ownership - no copy!
                currentFrameTimestamp = nextFrame.timestamp;
                renderedFrames++;
            }
        }
    }

    // Return clone to ensure data independence (UI will draw on it)
    // Still better than before: we only clone once here instead of multiple times
    return (currentFrame && !currentFrame->empty()) ? currentFrame->clone() : cv::Mat();
}


cv::Mat VideoModule::getCurrentFrameImmediate() {
    std::lock_guard<std::mutex> lock(currentFrameMutex);
    if (currentFrame && !currentFrame->empty()) {
        return currentFrame->clone();  // Clone for safety
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
    std::cout << "[SEEK] Seeking to " << targetTime << "s - flushing pipeline..." << std::endl;

    // Clear all pipeline queues (RAII handles cleanup)
    packetQueue->clear();
    frameQueue->clear();

    const int64_t ts = av_rescale_q(static_cast<int64_t>(targetTime * AV_TIME_BASE),
                            AV_TIME_BASE_Q,
                            formatContext->streams[streamIndex]->time_base);

    if (av_seek_frame(formatContext, streamIndex, ts, AVSEEK_FLAG_BACKWARD) >= 0) {
        avcodec_flush_buffers(codecContext);
        isBuffering = true;
        std::cout << "[SEEK] Seek complete, refilling pipeline..." << std::endl;
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
    stats.queueSize = frameQueue->size();
    stats.fps = fps;
    stats.currentTimestamp = currentFrameTimestamp;
    stats.hardwareAccelEnabled = (hwDeviceType != AV_HWDEVICE_TYPE_NONE);
    stats.decoderThreads = actualThreadCount;
    stats.verified = isVerified.load();
    stats.verificationError = verificationErrorMsg;

    if (stats.hardwareAccelEnabled) {
        stats.hwAccelType = av_hwdevice_get_type_name(hwDeviceType);
    }

    return stats;
}

bool VideoModule::isReadyToPlay() const {
    const size_t queueSize = frameQueue->size();
    const size_t maxSize = 120; // frameQueue max size
    return static_cast<double>(queueSize) / maxSize >= bufferFullnessThreshold;
}

bool VideoModule::needsBuffering() const {
    return isBuffering.load();
}

void VideoModule::waitForBuffer() {
    std::unique_lock<std::mutex> lock(bufferingMutex);
    bufferingCV.wait(lock, [this] {
        return !isBuffering.load() || shouldStop.load();
    });
}

void VideoModule::shutdown() {
    stopDecoding();

    // Stop verification thread if still running
    isVerifying = false;
    verificationCV.notify_all();
    if (verificationThread.joinable()) {
        verificationThread.join();
        std::cout << "  - Verification thread stopped" << std::endl;
    }

    // Queues cleared automatically by stopDecoding()

    if (codecContext) {
        avcodec_free_context(&codecContext);
        codecContext = nullptr;
    }

    if (hwDeviceCtx) {
        av_buffer_unref(&hwDeviceCtx);
        hwDeviceCtx = nullptr;
    }

    // Clean up HW frame pool
    {
        std::lock_guard<std::mutex> lock(hwFramePoolMutex);
        for (AVFrame* frame : hwFramePool) {
            av_frame_free(&frame);
        }
        hwFramePool.clear();
    }

    formatContext = nullptr;
    streamIndex = -1;
    hwDeviceType = AV_HWDEVICE_TYPE_NONE;
    useHardwareDecoding = false;
    isVerified = false;
}