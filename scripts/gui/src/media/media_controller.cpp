#include "media_controller.h"
#include <iostream>
#include <sstream>

MediaController::MediaController() 
    : formatContext(nullptr), audioStreamIndex(-1), videoStreamIndex(-1) {
    
    audioModule = std::make_unique<AudioModule>();
    videoModule = std::make_unique<VideoModule>();
}

MediaController::~MediaController() {
    shutdown();
}

bool MediaController::initialize(const AudioConfig& audioConfig, const VideoConfig& videoConfig) {
    std::lock_guard<std::mutex> lock(operationMutex);
    
    std::cout << "Initializing MediaController..." << std::endl;
    
    // Initialize FFmpeg
    av_log_set_level(AV_LOG_WARNING);
    
    if (!audioModule->initialize(audioConfig)) {
        setError("Failed to initialize audio module");
        return false;
    }
    
    if (!videoModule->initialize(videoConfig)) {
        setError("Failed to initialize video module");
        return false;
    }
    
    setState(MediaState::READY);
    std::cout << "MediaController initialized successfully" << std::endl;
    return true;
}

bool MediaController::loadFile(const std::string& filename) {
    std::lock_guard<std::mutex> lock(operationMutex);
    
    std::cout << "Loading media file: " << filename << std::endl;
    setState(MediaState::LOADING);
    
    // Close any existing file
    closeFile();
    
    if (!openFile(filename)) {
        setState(MediaState::ERROR);
        return false;
    }
    
    if (!findStreams()) {
        setError("No supported audio or video streams found");
        closeFile();
        setState(MediaState::ERROR);
        return false;
    }
    
    if (!loadStreams()) {
        setError("Failed to load media streams");
        closeFile();
        setState(MediaState::ERROR);
        return false;
    }
    
    updateMediaInfo();
    setState(MediaState::READY);
    
    std::cout << "Media file loaded successfully" << std::endl;
    std::cout << "Duration: " << mediaInfo.duration << "s" << std::endl;
    std::cout << "Has audio: " << (mediaInfo.hasAudio ? "Yes" : "No") << std::endl;
    std::cout << "Has video: " << (mediaInfo.hasVideo ? "Yes" : "No") << std::endl;
    
    return true;
}

bool MediaController::openFile(const std::string& filename) {
    formatContext = avformat_alloc_context();
    if (!formatContext) {
        setError("Failed to allocate format context");
        return false;
    }
    
    if (avformat_open_input(&formatContext, filename.c_str(), nullptr, nullptr) < 0) {
        setError("Could not open file: " + filename);
        avformat_free_context(formatContext);
        formatContext = nullptr;
        return false;
    }
    
    if (avformat_find_stream_info(formatContext, nullptr) < 0) {
        setError("Could not find stream information");
        avformat_close_input(&formatContext);
        return false;
    }
    
    return true;
}

bool MediaController::findStreams() {
    audioStreamIndex = videoStreamIndex = -1;
    
    for (unsigned int i = 0; i < formatContext->nb_streams; i++) {
        AVCodecParameters* codecpar = formatContext->streams[i]->codecpar;
        
        if (codecpar->codec_type == AVMEDIA_TYPE_AUDIO && audioStreamIndex == -1) {
            audioStreamIndex = i;
        } else if (codecpar->codec_type == AVMEDIA_TYPE_VIDEO && videoStreamIndex == -1) {
            videoStreamIndex = i;
        }
    }
    
    return (audioStreamIndex >= 0 || videoStreamIndex >= 0);
}

bool MediaController::loadStreams() {
    bool success = true;
    
    // Load audio stream if available
    if (audioStreamIndex >= 0) {
        if (!audioModule->loadStream(formatContext, audioStreamIndex)) {
            std::cerr << "Failed to load audio stream" << std::endl;
            success = false;
        }
    }
    
    // Load video stream if available
    if (videoStreamIndex >= 0) {
        if (!videoModule->loadStream(formatContext, videoStreamIndex)) {
            std::cerr << "Failed to load video stream" << std::endl;
            success = false;
        }
    }
    
    return success;
}

void MediaController::updateMediaInfo() {
    std::lock_guard<std::mutex> lock(mediaInfoMutex);
    
    mediaInfo.hasAudio = (audioStreamIndex >= 0);
    mediaInfo.hasVideo = (videoStreamIndex >= 0);
    
    if (formatContext) {
        mediaInfo.duration = formatContext->duration / static_cast<double>(AV_TIME_BASE);
        
        if (mediaInfo.hasVideo) {
            mediaInfo.videoWidth = videoModule->getWidth();
            mediaInfo.videoHeight = videoModule->getHeight();
            mediaInfo.fps = videoModule->getFPS();
            
            // Get video codec name
            AVCodecParameters* vcodecpar = formatContext->streams[videoStreamIndex]->codecpar;
            const AVCodec* vcodec = avcodec_find_decoder(vcodecpar->codec_id);
            if (vcodec) {
                mediaInfo.videoCodec = vcodec->name;
            }
        }
        
        if (mediaInfo.hasAudio) {
            AVCodecParameters* acodecpar = formatContext->streams[audioStreamIndex]->codecpar;
            mediaInfo.audioSampleRate = acodecpar->sample_rate;
            mediaInfo.audioChannels = acodecpar->ch_layout.nb_channels;
            
            // Get audio codec name
            const AVCodec* acodec = avcodec_find_decoder(acodecpar->codec_id);
            if (acodec) {
                mediaInfo.audioCodec = acodec->name;
            }
        }
    }
}

void MediaController::closeFile() {
    if (formatContext) {
        avformat_close_input(&formatContext);
        formatContext = nullptr;
    }
    
    audioStreamIndex = -1;
    videoStreamIndex = -1;
    
    // Reset media info
    mediaInfo = MediaInfo();
}

bool MediaController::play() {
    std::lock_guard<std::mutex> lock(operationMutex);
    
    if (currentState != MediaState::READY && currentState != MediaState::PAUSED) {
        setError("Cannot play: media not ready");
        return false;
    }
    
    std::cout << "Starting playback..." << std::endl;
    
    // Start video decoding first (if available)
    if (hasVideo()) {
        videoModule->startDecoding();
    }
    
    // Start audio playback (master clock)
    if (hasAudio()) {
        audioModule->play();
    }
    
    setState(MediaState::PLAYING);
    std::cout << "Playback started" << std::endl;
    return true;
}

bool MediaController::pause() {
    std::lock_guard<std::mutex> lock(operationMutex);
    
    if (currentState != MediaState::PLAYING) {
        return false;
    }
    
    std::cout << "Pausing playback..." << std::endl;
    
    // Pause audio first (master clock)
    if (hasAudio()) {
        audioModule->pause();
    }
    
    // Stop video decoding
    if (hasVideo()) {
        videoModule->stopDecoding();
    }
    
    setState(MediaState::PAUSED);
    std::cout << "Playback paused at " << getCurrentTime() << "s" << std::endl;
    return true;
}

bool MediaController::stop() {
    std::lock_guard<std::mutex> lock(operationMutex);
    
    if (currentState == MediaState::STOPPED) {
        return true;
    }
    
    std::cout << "Stopping playback..." << std::endl;
    
    // Stop audio
    if (hasAudio()) {
        audioModule->stop();
    }
    
    // Stop video
    if (hasVideo()) {
        videoModule->stopDecoding();
    }
    
    setState(MediaState::READY);
    std::cout << "Playback stopped" << std::endl;
    return true;
}

bool MediaController::seek(double timeSeconds) {
    std::lock_guard<std::mutex> lock(operationMutex);
    
    if (!formatContext || timeSeconds < 0 || timeSeconds > getDuration()) {
        setError("Invalid seek time");
        return false;
    }
    
    std::cout << "Seeking to " << timeSeconds << "s..." << std::endl;
    setState(MediaState::SEEKING);
    
    bool wasPlaying = (currentState == MediaState::PLAYING);
    
    // Pause playback during seek
    if (hasAudio()) {
        audioModule->pause();
    }
    if (hasVideo()) {
        videoModule->stopDecoding();
    }
    
    // Perform seek operations
    bool success = true;
    
    if (hasAudio()) {
        if (!audioModule->seek(timeSeconds)) {
            std::cerr << "Audio seek failed" << std::endl;
            success = false;
        }
    }
    
    if (hasVideo()) {
        if (!videoModule->seek(timeSeconds)) {
            std::cerr << "Video seek failed" << std::endl;
            success = false;
        }
    }
    
    if (!success) {
        setError("Seek operation failed");
        setState(MediaState::ERROR);
        return false;
    }
    
    // Resume playback if we were playing
    if (wasPlaying) {
        if (hasVideo()) {
            videoModule->startDecoding();
        }
        if (hasAudio()) {
            audioModule->play();
        }
        setState(MediaState::PLAYING);
    } else {
        setState(MediaState::PAUSED);
    }
    
    std::cout << "Seek completed to " << timeSeconds << "s" << std::endl;
    return true;
}

cv::Mat MediaController::getCurrentVideoFrame() {
    if (!hasVideo() || currentState == MediaState::STOPPED) {
        return cv::Mat();
    }
    
    if (syncEnabled && hasAudio()) {
        // Sync video to audio master clock
        double audioTime = audioModule->getCurrentTime();
        return videoModule->getCurrentFrame(audioTime);
    } else {
        // Return immediate frame without sync
        return videoModule->getCurrentFrameImmediate();
    }
}

void MediaController::setVolume(float volume) {
    if (hasAudio()) {
        audioModule->setVolume(volume);
    }
}

float MediaController::getVolume() const {
    if (hasAudio()) {
        return audioModule->getVolume();
    }
    return 0.0f;
}

double MediaController::getCurrentTime() const {
    if (hasAudio()) {
        return audioModule->getCurrentTime();
    } else if (hasVideo()) {
        return videoModule->getStats().currentTimestamp;
    }
    return 0.0;
}

double MediaController::getDuration() const {
    std::lock_guard<std::mutex> lock(mediaInfoMutex);
    return mediaInfo.duration;
}

MediaState MediaController::getState() const {
    return currentState.load();
}

MediaInfo MediaController::getMediaInfo() const {
    std::lock_guard<std::mutex> lock(mediaInfoMutex);
    return mediaInfo;
}

MediaStats MediaController::getStats() const {
    MediaStats stats;
    stats.state = getState();
    stats.currentTime = getCurrentTime();
    
    if (hasAudio()) {
        stats.audioStats = audioModule->getStats();
    }
    
    if (hasVideo()) {
        stats.videoStats = videoModule->getStats();
    }
    
    if (syncEnabled && hasAudio() && hasVideo()) {
        stats.syncOffset = calculateSyncOffset();
    }
    
    return stats;
}

bool MediaController::hasAudio() const {
    return audioStreamIndex >= 0 && audioModule->isStreamLoaded();
}

bool MediaController::hasVideo() const {
    return videoStreamIndex >= 0 && videoModule->isStreamLoaded();
}

void MediaController::enableSync(bool enable) {
    syncEnabled = enable;
    std::cout << "A/V sync " << (enable ? "enabled" : "disabled") << std::endl;
}

bool MediaController::isSyncEnabled() const {
    return syncEnabled.load();
}

void MediaController::setMaxSyncOffset(double offsetSeconds) {
    maxSyncOffset = offsetSeconds;
}

double MediaController::getMaxSyncOffset() const {
    return maxSyncOffset.load();
}

double MediaController::calculateSyncOffset() const {
    if (!hasAudio() || !hasVideo()) return 0.0;
    
    double audioTime = audioModule->getCurrentTime();
    double videoTime = videoModule->getStats().currentTimestamp;
    
    return videoTime - audioTime; // positive = video ahead, negative = video behind
}

std::string MediaController::getLastError() const {
    return lastError;
}

void MediaController::setError(const std::string& error) {
    lastError = error;
    std::cerr << "MediaController Error: " << error << std::endl;
}

void MediaController::setState(MediaState newState) {
    currentState = newState;
}

void MediaController::unloadFile() {
    std::lock_guard<std::mutex> lock(operationMutex);
    
    stop();
    closeFile();
    setState(MediaState::READY);
    
    std::cout << "Media file unloaded" << std::endl;
}

void MediaController::shutdown() {
    std::cout << "Shutting down MediaController..." << std::endl;
    
    stop();
    closeFile();
    
    if (audioModule) {
        audioModule->shutdown();
    }
    
    if (videoModule) {
        videoModule->shutdown();
    }
    
    setState(MediaState::STOPPED);
}