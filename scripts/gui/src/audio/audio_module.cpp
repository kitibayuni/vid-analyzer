#include "audio_module.h"
#include <algorithm>
#include <cstring>
#include <iostream>
#include <vector>

// AudioModule implementation
AudioModule::AudioModule()
    : audioDevice(0), formatContext(nullptr), codecContext(nullptr),
      swrContext(nullptr), streamIndex(-1) {
  lastClockUpdate = std::chrono::steady_clock::now();
}

AudioModule::~AudioModule() { shutdown(); }

bool AudioModule::initialize(const AudioConfig &config) {
  std::cout << "Initializing AudioModule (will configure based on codec)..." << std::endl;

  // SDL will be initialized when we load the stream and know the codec params
  // Just store the config for now
  return true;
}

bool AudioModule::initializeSDL(int sampleRate, int channels, int bufferSize) {
  // Close existing device if already open
  if (audioDevice != 0) {
    SDL_CloseAudioDevice(audioDevice);
    audioDevice = 0;
  }

  if (SDL_Init(SDL_INIT_AUDIO) < 0) {
    std::cerr << "SDL initialization failed: " << SDL_GetError() << std::endl;
    return false;
  }

  SDL_AudioSpec desired;
  SDL_zero(desired);
  desired.freq = sampleRate;
  desired.format = AUDIO_S16SYS;  // Fixed S16 format
  desired.channels = channels;
  desired.samples = bufferSize;
  desired.callback = nullptr;  // Using queue audio instead of callback

  audioDevice = SDL_OpenAudioDevice(nullptr, 0, &desired, &audioSpec, 0);

  if (audioDevice == 0) {
    std::cerr << "Failed to open audio device: " << SDL_GetError() << std::endl;
    SDL_Quit();
    return false;
  }

  // Start with audio paused
  SDL_PauseAudioDevice(audioDevice, 1);

  std::cout << "SDL Audio initialized: " << audioSpec.freq << "Hz, "
            << static_cast<int>(audioSpec.channels) << " channels, S16 format" << std::endl;

  return true;
}

bool AudioModule::loadStream(AVFormatContext* ctx, int audioStreamIndex) {
    if (!ctx) {
        std::cerr << "AudioModule: Null format context" << std::endl;
        return false;
    }
    
    if (audioStreamIndex < 0 || audioStreamIndex >= static_cast<int>(ctx->nb_streams)) {
        std::cerr << "AudioModule: Invalid stream index " << audioStreamIndex 
                  << " (total streams: " << ctx->nb_streams << ")" << std::endl;
        return false;
    }
    
    AVCodecParameters* codecpar = ctx->streams[audioStreamIndex]->codecpar;
    if (codecpar->codec_type != AVMEDIA_TYPE_AUDIO) {
        std::cerr << "AudioModule: Stream " << audioStreamIndex 
                  << " is not an audio stream" << std::endl;
        return false;
    }
    
    std::cout << "Loading audio stream " << audioStreamIndex << std::endl;

  formatContext = ctx;
  streamIndex = audioStreamIndex;

  AVCodecParameters *codecParams =
      formatContext->streams[streamIndex]->codecpar;
  const AVCodec *codec = avcodec_find_decoder(codecParams->codec_id);

  if (!codec) {
    std::cerr << "Audio codec not found" << std::endl;
    return false;
  }

  codecContext = avcodec_alloc_context3(codec);
  if (!codecContext) {
    std::cerr << "Failed to allocate audio codec context" << std::endl;
    return false;
  }

  if (avcodec_parameters_to_context(codecContext, codecParams) < 0) {
    std::cerr << "Failed to copy audio codec parameters" << std::endl;
    avcodec_free_context(&codecContext);
    return false;
  }

  if (avcodec_open2(codecContext, codec, nullptr) < 0) {
    std::cerr << "Failed to open audio codec" << std::endl;
    avcodec_free_context(&codecContext);
    return false;
  }

  // Initialize SDL with codec's native parameters (using S16 format)
  std::cout << "Using S16 audio format" << std::endl;

  if (!initializeSDL(codecContext->sample_rate,
                     codecContext->ch_layout.nb_channels,
                     4096)) {  // Buffer size (4096 samples for stable buffering)
    std::cerr << "Failed to initialize SDL audio" << std::endl;
    avcodec_free_context(&codecContext);
    return false;
  }

  if (!setupResampler()) {
    std::cerr << "Failed to setup audio resampler" << std::endl;
    avcodec_free_context(&codecContext);
    return false;
  }

  // Reset state
  clockTime = 0.0;
  totalDecodedTime = 0.0;
  underrunCount = 0;
  overrunCount = 0;
  audioStarted = false;
  endOfFile = false;

  // Clear any queued audio
  SDL_ClearQueuedAudio(audioDevice);

  // Start decode thread
  shouldStop = false;
  decodeThread = std::thread(&AudioModule::decodeThreadFunction, this);

  std::cout << "Audio stream loaded successfully" << std::endl;
  return true;
}

bool AudioModule::setupResampler() {
  swrContext = swr_alloc();
  if (!swrContext)
    return false;

  // Get channel count using new API
  int nb_channels = codecContext->ch_layout.nb_channels;

  AVChannelLayout out_ch_layout = AV_CHANNEL_LAYOUT_STEREO;
  if (nb_channels == 1) {
    out_ch_layout = AV_CHANNEL_LAYOUT_MONO;
  }

  av_opt_set_chlayout(swrContext, "in_chlayout", &codecContext->ch_layout, 0);
  av_opt_set_chlayout(swrContext, "out_chlayout", &out_ch_layout, 0);
  av_opt_set_int(swrContext, "in_sample_rate", codecContext->sample_rate, 0);
  av_opt_set_int(swrContext, "out_sample_rate", codecContext->sample_rate, 0);
  av_opt_set_sample_fmt(swrContext, "in_sample_fmt", codecContext->sample_fmt, 0);
  av_opt_set_sample_fmt(swrContext, "out_sample_fmt", AV_SAMPLE_FMT_S16, 0);

  if (swr_init(swrContext) < 0) {
    std::cerr << "Could not initialize resampler" << std::endl;
    swr_free(&swrContext);
    return false;
  }

  return true;
}

void AudioModule::decodeThreadFunction() {
    if (!swrContext || streamIndex < 0) return;

    AVPacket* pkt = av_packet_alloc();
    AVFrame* frame = av_frame_alloc();

    if (!pkt || !frame) {
        std::cerr << "Could not allocate packet or frame" << std::endl;
        if (pkt) av_packet_free(&pkt);
        if (frame) av_frame_free(&frame);
        return;
    }

    int nb_channels = codecContext->ch_layout.nb_channels;

    // Main decoding loop
    while (!shouldStop && !endOfFile) {
        // Check current buffer level
        uint32_t queued_audio_size = SDL_GetQueuedAudioSize(audioDevice);

        // If buffer is getting full, wait a bit
        if (queued_audio_size > MAX_BUFFER_SIZE) {
            SDL_Delay(10);
            continue;
        }

        // Start audio playback once we have enough data
        if (!audioStarted && queued_audio_size >= MIN_BUFFER_SIZE && isPlaying) {
            SDL_PauseAudioDevice(audioDevice, 0);
            audioStarted = true;
            std::cout << "Audio playback started" << std::endl;
        }

        // Only decode if playing
        if (!isPlaying) {
            SDL_Delay(10);
            continue;
        }

        // Read next packet
        int ret = av_read_frame(formatContext, pkt);
        if (ret < 0) {
            if (ret == AVERROR_EOF) {
                // Send flush packet to decoder
                avcodec_send_packet(codecContext, nullptr);
                endOfFile = true;
            } else {
                std::cerr << "Error reading frame" << std::endl;
                break;
            }
        }

        // Skip non-audio packets
        if (!endOfFile && pkt->stream_index != streamIndex) {
            av_packet_unref(pkt);
            continue;
        }

        // Send packet to decoder
        if (!endOfFile) {
            ret = avcodec_send_packet(codecContext, pkt);
            if (ret < 0 && ret != AVERROR(EAGAIN)) {
                std::cerr << "Error sending packet to decoder" << std::endl;
                av_packet_unref(pkt);
                continue;
            }
        }

        // Receive all available frames from decoder
        while (true) {
            ret = avcodec_receive_frame(codecContext, frame);
            if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                break;
            }
            if (ret < 0) {
                std::cerr << "Error receiving frame from decoder" << std::endl;
                break;
            }

            // Calculate output buffer size
            int out_samples = swr_get_out_samples(swrContext, frame->nb_samples);
            if (out_samples <= 0) {
                continue;
            }

            // Allocate output buffer
            uint8_t* out_buffer[2] = {nullptr};
            int out_linesize;
            ret = av_samples_alloc(out_buffer, &out_linesize, nb_channels,
                                  out_samples, AV_SAMPLE_FMT_S16, 0);
            if (ret < 0) {
                std::cerr << "Could not allocate output buffer" << std::endl;
                continue;
            }

            // Convert audio format
            int converted_samples = swr_convert(swrContext, out_buffer, out_samples,
                                               (const uint8_t**)frame->data, frame->nb_samples);

            if (converted_samples > 0) {
                int audio_bytes = converted_samples * nb_channels * sizeof(int16_t);

                // Apply volume
                float vol = volume.load();
                int16_t* samples = reinterpret_cast<int16_t*>(out_buffer[0]);
                int num_samples = converted_samples * nb_channels;
                for (int i = 0; i < num_samples; i++) {
                    int32_t temp = static_cast<int32_t>(samples[i] * vol);
                    // Clamp to prevent distortion
                    if (temp > 32767) temp = 32767;
                    if (temp < -32768) temp = -32768;
                    samples[i] = static_cast<int16_t>(temp);
                }

                // Queue the audio data
                if (SDL_QueueAudio(audioDevice, out_buffer[0], audio_bytes) < 0) {
                    std::cerr << "SDL_QueueAudio error: " << SDL_GetError() << std::endl;
                } else {
                    // Track total decoded time for accurate clock synchronization
                    double frame_duration = static_cast<double>(converted_samples) / audioSpec.freq;
                    totalDecodedTime.store(totalDecodedTime.load() + frame_duration);
                }
            }

            // Free the output buffer
            av_freep(&out_buffer[0]);
        }

        if (!endOfFile) {
            av_packet_unref(pkt);
        }
    }

    // Flush the resampler
    if (!shouldStop) {
        std::cout << "Flushing resampler..." << std::endl;
        while (true) {
            uint8_t* out_buffer[2] = {nullptr};
            int out_samples = 4096;
            int out_linesize;

            av_samples_alloc(out_buffer, &out_linesize, nb_channels,
                            out_samples, AV_SAMPLE_FMT_S16, 0);

            int converted_samples = swr_convert(swrContext, out_buffer, out_samples, nullptr, 0);

            if (converted_samples <= 0) {
                av_freep(&out_buffer[0]);
                break;
            }

            int audio_bytes = converted_samples * nb_channels * sizeof(int16_t);

            // Apply volume
            float vol = volume.load();
            int16_t* samples = reinterpret_cast<int16_t*>(out_buffer[0]);
            int num_samples = converted_samples * nb_channels;
            for (int i = 0; i < num_samples; i++) {
                int32_t temp = static_cast<int32_t>(samples[i] * vol);
                if (temp > 32767) temp = 32767;
                if (temp < -32768) temp = -32768;
                samples[i] = static_cast<int16_t>(temp);
            }

            if (SDL_QueueAudio(audioDevice, out_buffer[0], audio_bytes) == 0) {
                // Track flushed audio time
                double frame_duration = static_cast<double>(converted_samples) / audioSpec.freq;
                totalDecodedTime.store(totalDecodedTime.load() + frame_duration);
            }
            av_freep(&out_buffer[0]);
        }
    }

    av_frame_free(&frame);
    av_packet_free(&pkt);
}


void AudioModule::play() {
  if (!isInitialized() || !isStreamLoaded())
    return;

  isPlaying = true;
  SDL_PauseAudioDevice(audioDevice, 0);
  lastClockUpdate = std::chrono::steady_clock::now();

  std::cout << "Audio playing" << std::endl;
}

void AudioModule::pause() {
  if (!isInitialized())
    return;

  isPlaying = false;
  SDL_PauseAudioDevice(audioDevice, 1);

  std::cout << "Audio paused" << std::endl;
}

void AudioModule::stop() {
  pause();
  clockTime = 0.0;
  totalDecodedTime = 0.0;
  audioStarted = false;
  SDL_ClearQueuedAudio(audioDevice);
}

bool AudioModule::seek(double timeSeconds) {
  if (!isStreamLoaded() || !formatContext)
    return false;

  std::cout << "Audio seeking to: " << timeSeconds << "s" << std::endl;

  bool wasPlaying = isPlaying;
  pause();

  // Clear queued audio
  SDL_ClearQueuedAudio(audioDevice);
  audioStarted = false;
  endOfFile = false;

  // Seek in format context
  int64_t ts = av_rescale_q(static_cast<int64_t>(timeSeconds * AV_TIME_BASE),
                            AV_TIME_BASE_Q,
                            formatContext->streams[streamIndex]->time_base);

  if (av_seek_frame(formatContext, streamIndex, ts, AVSEEK_FLAG_BACKWARD) < 0) {
    std::cerr << "Audio seek failed" << std::endl;
    return false;
  }

  // Flush codec buffers
  avcodec_flush_buffers(codecContext);

  // Update clock and reset decoded time tracking
  clockTime = timeSeconds;
  totalDecodedTime = 0.0;
  lastClockUpdate = std::chrono::steady_clock::now();

  if (wasPlaying) {
    play();
  }

  return true;
}

void AudioModule::setVolume(float vol) {
  if (vol < 0.0f)
    vol = 0.0f;
  if (vol > 1.0f)
    vol = 1.0f;
  volume = vol;
}

float AudioModule::getVolume() const { return volume.load(); }

double AudioModule::getCurrentTime() const {
  if (!isInitialized() || !audioStarted.load()) {
    return clockTime.load();
  }

  // Queue-based clock synchronization:
  // Current playback time = total decoded time - buffered (queued) time
  double decoded_time = totalDecodedTime.load();

  // Calculate how much audio is buffered (not yet played)
  uint32_t queued_bytes = SDL_GetQueuedAudioSize(audioDevice);
  double buffered_time = static_cast<double>(queued_bytes) /
                         (audioSpec.freq * audioSpec.channels * sizeof(int16_t));

  // Actual playback position
  double playback_time = clockTime.load() + (decoded_time - buffered_time);

  return playback_time;
}

void AudioModule::setCurrentTime(double timeSeconds) {
  clockTime = timeSeconds;
  lastClockUpdate = std::chrono::steady_clock::now();
}

bool AudioModule::isInitialized() const { return audioDevice != 0; }

bool AudioModule::isStreamLoaded() const {
  return codecContext != nullptr && swrContext != nullptr;
}

bool AudioModule::getIsPlaying() const { return isPlaying.load(); }

AudioStats AudioModule::getStats() const {
  AudioStats stats;

  // Calculate buffer fullness based on queued audio
  if (isInitialized()) {
    uint32_t queued = SDL_GetQueuedAudioSize(audioDevice);
    stats.bufferFullness = static_cast<double>(queued) / MAX_BUFFER_SIZE;
  } else {
    stats.bufferFullness = 0.0;
  }

  stats.underruns = underrunCount.load();
  stats.overruns = overrunCount.load();
  stats.currentTime = getCurrentTime();
  stats.isPlaying = isPlaying.load();
  return stats;
}

void AudioModule::shutdown() {
  std::cout << "Shutting down AudioModule..." << std::endl;

  // Display final statistics
  std::cout << "Audio stats - Underruns: " << underrunCount.load()
            << ", Overruns: " << overrunCount.load() << std::endl;

  shouldStop = true;

  if (decodeThread.joinable()) {
    decodeThread.join();
  }

  if (audioDevice != 0) {
    SDL_ClearQueuedAudio(audioDevice);
    SDL_CloseAudioDevice(audioDevice);
    audioDevice = 0;
  }

  cleanup();
  SDL_Quit();
}

void AudioModule::cleanup() {
  if (swrContext) {
    swr_free(&swrContext);
  }
  if (codecContext) {
    avcodec_free_context(&codecContext);
  }

  formatContext = nullptr;
  streamIndex = -1;
}