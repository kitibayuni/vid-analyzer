#include "audio_module.h"
#include <algorithm>
#include <iostream>
#include <vector>

// Optimized circular audio buffer implementation
class CircularAudioBuffer {
private:
  std::vector<int16_t> buffer;
  std::atomic<size_t> write_pos{0};
  std::atomic<size_t> read_pos{0};
  size_t capacity;

public:
  CircularAudioBuffer(size_t size) : capacity(size) { buffer.resize(capacity); }

  bool write(const int16_t *data, size_t samples) {
    size_t write_idx = write_pos.load(std::memory_order_relaxed);
    size_t read_idx = read_pos.load(std::memory_order_acquire);

    size_t available = (read_idx > write_idx)
                           ? (read_idx - write_idx - 1)
                           : (capacity - write_idx + read_idx - 1);

    if (available < samples)
      return false;

    for (size_t i = 0; i < samples; i++) {
      buffer[write_idx] = data[i];
      write_idx = (write_idx + 1) % capacity;
    }

    write_pos.store(write_idx, std::memory_order_release);
    return true;
  }

  size_t read(int16_t *data, size_t max_samples) {
    size_t write_idx = write_pos.load(std::memory_order_acquire);
    size_t read_idx = read_pos.load(std::memory_order_relaxed);

    size_t available = (write_idx >= read_idx)
                           ? (write_idx - read_idx)
                           : (capacity - read_idx + write_idx);

    size_t to_read = std::min(available, max_samples);

    for (size_t i = 0; i < to_read; i++) {
      data[i] = buffer[read_idx];
      read_idx = (read_idx + 1) % capacity;
    }

    read_pos.store(read_idx, std::memory_order_release);
    return to_read;
  }

  void clear() {
    write_pos.store(0, std::memory_order_relaxed);
    read_pos.store(0, std::memory_order_relaxed);
  }

  size_t available() const {
    size_t write_idx = write_pos.load(std::memory_order_relaxed);
    size_t read_idx = read_pos.load(std::memory_order_relaxed);
    return (write_idx >= read_idx) ? (write_idx - read_idx)
                                   : (capacity - read_idx + write_idx);
  }

  double fullness_ratio() const {
    return static_cast<double>(available()) / capacity;
  }
};

// AudioBuffers implementation
AudioModule::AudioBuffers::AudioBuffers()
    : output_data(nullptr), output_linesize(0), max_samples(0) {}

AudioModule::AudioBuffers::~AudioBuffers() {
  if (output_data) {
    av_freep(&output_data[0]);
    av_freep(&output_data);
  }
}

bool AudioModule::AudioBuffers::allocate(int samples, int channels) {
  if (samples <= max_samples && output_data)
    return true;

  if (output_data) {
    av_freep(&output_data[0]);
    av_freep(&output_data);
  }

  int ret = av_samples_alloc_array_and_samples(
      &output_data, &output_linesize, channels, samples, AV_SAMPLE_FMT_S16, 0);
  if (ret < 0)
    return false;

  max_samples = samples;
  return true;
}

// AudioModule implementation
AudioModule::AudioModule()
    : audioDevice(0), formatContext(nullptr), codecContext(nullptr),
      swrContext(nullptr), streamIndex(-1) {
  lastClockUpdate = std::chrono::steady_clock::now();
}

AudioModule::~AudioModule() { shutdown(); }

bool AudioModule::initialize(const AudioConfig &config) {
  std::cout << "Initializing AudioModule..." << std::endl;

  if (!initializeSDL(config)) {
    std::cerr << "Failed to initialize SDL audio" << std::endl;
    return false;
  }

  // Create circular buffer based on configuration
  size_t bufferSize =
      config.sampleRate * config.channels * config.bufferDurationSeconds;
  audioBuffer.reset(new CircularAudioBuffer(bufferSize));

  std::cout << "AudioModule initialized successfully" << std::endl;
  std::cout << "Audio config: " << audioSpec.freq << "Hz, "
            << static_cast<int>(audioSpec.channels) << " channels, "
            << "buffer: " << config.bufferDurationSeconds << "s" << std::endl;

  return true;
}

bool AudioModule::initializeSDL(const AudioConfig &config) {
  if (SDL_Init(SDL_INIT_AUDIO) < 0) {
    std::cerr << "SDL initialization failed: " << SDL_GetError() << std::endl;
    return false;
  }

  SDL_AudioSpec desired;
  desired.freq = config.sampleRate;
  desired.format = AUDIO_S16LSB;
  desired.channels = config.channels;
  desired.samples = config.bufferSize;
  desired.callback = audioCallback;
  desired.userdata = this;

  audioDevice = SDL_OpenAudioDevice(nullptr, 0, &desired, &audioSpec, 0);

  if (audioDevice == 0) {
    std::cerr << "Failed to open audio device: " << SDL_GetError() << std::endl;
    SDL_Quit();
    return false;
  }

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

  if (!setupResampler()) {
    std::cerr << "Failed to setup audio resampler" << std::endl;
    avcodec_free_context(&codecContext);
    return false;
  }

  // Clear buffer and reset clock
  audioBuffer->clear();
  clockTime = 0.0;
  underrunCount = 0;
  overrunCount = 0;

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

  AVChannelLayout input_ch_layout, output_ch_layout;

  if (codecContext->ch_layout.nb_channels > 0) {
    av_channel_layout_copy(&input_ch_layout, &codecContext->ch_layout);
  } else {
    av_channel_layout_default(&input_ch_layout,
                              codecContext->ch_layout.nb_channels);
  }

  av_channel_layout_default(&output_ch_layout, audioSpec.channels);

  av_opt_set_chlayout(swrContext, "in_chlayout", &input_ch_layout, 0);
  av_opt_set_int(swrContext, "in_sample_rate", codecContext->sample_rate, 0);
  av_opt_set_sample_fmt(swrContext, "in_sample_fmt", codecContext->sample_fmt,
                        0);

  av_opt_set_chlayout(swrContext, "out_chlayout", &output_ch_layout, 0);
  av_opt_set_int(swrContext, "out_sample_rate", audioSpec.freq, 0);
  av_opt_set_sample_fmt(swrContext, "out_sample_fmt", AV_SAMPLE_FMT_S16, 0);

  int result = swr_init(swrContext);

  av_channel_layout_uninit(&input_ch_layout);
  av_channel_layout_uninit(&output_ch_layout);

  return result >= 0;
}

void AudioModule::audioCallback(void *userdata, Uint8 *stream, int len) {
  AudioModule *module = static_cast<AudioModule *>(userdata);

  SDL_memset(stream, 0, len);

  int16_t *output = reinterpret_cast<int16_t *>(stream);
  int samples_needed = len / sizeof(int16_t);

  int samples_read = module->audioBuffer->read(output, samples_needed);

  if (samples_read < samples_needed) {
    module->underrunCount++;
  }

  // Update master clock
  if (samples_read > 0) {
    double time_per_sample =
        1.0 / (module->audioSpec.freq * module->audioSpec.channels);
    double audio_time_advance = samples_read * time_per_sample;
    module->clockTime.store(module->clockTime.load() + audio_time_advance);
    module->lastClockUpdate = std::chrono::steady_clock::now();
  }

  // Apply volume
  float vol = module->volume.load();
  for (int i = 0; i < samples_read; i++) {
    output[i] = static_cast<int16_t>(output[i] * vol);
  }
}

void AudioModule::decodeThreadFunction() {
    if (!swrContext || streamIndex < 0) return;
    
    AVPacket* packet = av_packet_alloc();
    AVFrame* frame = av_frame_alloc();
    
    while (!shouldStop) {
        if (isPlaying && audioBuffer->fullness_ratio() < 0.8) {
            if (av_read_frame(formatContext, packet) >= 0) {
                if (packet->stream_index == streamIndex) {
                    if (avcodec_send_packet(codecContext, packet) >= 0) {
                        while (avcodec_receive_frame(codecContext, frame) >= 0) {
                            int output_samples = swr_get_out_samples(swrContext, frame->nb_samples);
                            if (output_samples > 0) {
                                if (audioBuffers.allocate(output_samples, audioSpec.channels)) {
                                    int resampled = swr_convert(swrContext, audioBuffers.output_data, output_samples,
                                                              (const uint8_t**)frame->data, frame->nb_samples);
                                    
                                    if (resampled > 0) {
                                        int16_t* samples = reinterpret_cast<int16_t*>(audioBuffers.output_data[0]);
                                        int total_samples = resampled * audioSpec.channels;
                                        
                                        if (!audioBuffer->write(samples, total_samples)) {
                                            overrunCount++;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                av_packet_unref(packet);
            } else {
                // EOF - could seek back to start for looping
                break;
            }
        } else {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
    }
    
    av_frame_free(&frame);
    av_packet_free(&packet);
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
  if (audioBuffer) {
    audioBuffer->clear();
  }
}

bool AudioModule::seek(double timeSeconds) {
  if (!isStreamLoaded() || !formatContext)
    return false;

  std::cout << "Audio seeking to: " << timeSeconds << "s" << std::endl;

  bool wasPlaying = isPlaying;
  pause();

  // Clear buffer
  audioBuffer->clear();

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

  // Update clock
  clockTime = timeSeconds;
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
  double base_time = clockTime.load();

  if (isPlaying) {
    auto now = std::chrono::steady_clock::now();
    auto elapsed = std::chrono::duration<double>(now - lastClockUpdate).count();
    return base_time + elapsed;
  }

  return base_time;
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
  stats.bufferFullness = audioBuffer ? audioBuffer->fullness_ratio() : 0.0;
  stats.underruns = underrunCount.load();
  stats.overruns = overrunCount.load();
  stats.currentTime = getCurrentTime();
  stats.isPlaying = isPlaying.load();
  return stats;
}

void AudioModule::shutdown() {
  std::cout << "Shutting down AudioModule..." << std::endl;

  shouldStop = true;

  if (decodeThread.joinable()) {
    decodeThread.join();
  }

  if (audioDevice != 0) {
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