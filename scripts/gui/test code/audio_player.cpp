#include <iostream>
#include <thread>
#include <chrono>

extern "C" {
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include <libswresample/swresample.h>
#include <libavutil/opt.h>
#include <SDL2/SDL.h>
}

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <file.mp4>\n";
        return -1;
    }

    const char* filename = argv[1];

    avformat_network_init();

    AVFormatContext* fmt_ctx = nullptr;
    if (avformat_open_input(&fmt_ctx, filename, nullptr, nullptr) < 0) {
        std::cerr << "Could not open file\n";
        return -1;
    }

    if (avformat_find_stream_info(fmt_ctx, nullptr) < 0) {
        std::cerr << "Could not find stream info\n";
        avformat_close_input(&fmt_ctx);
        return -1;
    }

    // Find the audio stream
    int audio_stream_index = -1;
    for (unsigned i = 0; i < fmt_ctx->nb_streams; ++i) {
        if (fmt_ctx->streams[i]->codecpar->codec_type == AVMEDIA_TYPE_AUDIO) {
            audio_stream_index = i;
            break;
        }
    }
    
    if (audio_stream_index == -1) {
        std::cerr << "No audio stream found\n";
        avformat_close_input(&fmt_ctx);
        return -1;
    }

    AVCodecParameters* codecpar = fmt_ctx->streams[audio_stream_index]->codecpar;
    const AVCodec* codec = avcodec_find_decoder(codecpar->codec_id);
    if (!codec) {
        std::cerr << "Codec not found\n";
        avformat_close_input(&fmt_ctx);
        return -1;
    }

    AVCodecContext* codec_ctx = avcodec_alloc_context3(codec);
    if (!codec_ctx) {
        std::cerr << "Could not allocate codec context\n";
        avformat_close_input(&fmt_ctx);
        return -1;
    }
    
    if (avcodec_parameters_to_context(codec_ctx, codecpar) < 0) {
        std::cerr << "Could not copy codec parameters\n";
        avcodec_free_context(&codec_ctx);
        avformat_close_input(&fmt_ctx);
        return -1;
    }
    
    if (avcodec_open2(codec_ctx, codec, nullptr) < 0) {
        std::cerr << "Could not open codec\n";
        avcodec_free_context(&codec_ctx);
        avformat_close_input(&fmt_ctx);
        return -1;
    }

    // Get channel count using new API
    int nb_channels = codec_ctx->ch_layout.nb_channels;
    
    // Initialize SDL2 audio
    if (SDL_Init(SDL_INIT_AUDIO) < 0) {
        std::cerr << "SDL_Init Error: " << SDL_GetError() << "\n";
        avcodec_free_context(&codec_ctx);
        avformat_close_input(&fmt_ctx);
        return -1;
    }

    SDL_AudioSpec wanted_spec, obtained_spec;
    SDL_zero(wanted_spec);
    wanted_spec.freq = codec_ctx->sample_rate;
    wanted_spec.format = AUDIO_S16SYS;
    wanted_spec.channels = nb_channels;
    wanted_spec.samples = 2048; // Smaller buffer for lower latency
    wanted_spec.callback = nullptr; // Using queue audio
    
    SDL_AudioDeviceID audio_dev = SDL_OpenAudioDevice(nullptr, 0, &wanted_spec, &obtained_spec, 0);
    if (audio_dev == 0) {
        std::cerr << "SDL_OpenAudioDevice Error: " << SDL_GetError() << "\n";
        avcodec_free_context(&codec_ctx);
        avformat_close_input(&fmt_ctx);
        SDL_Quit();
        return -1;
    }
    
    // Start with audio paused
    SDL_PauseAudioDevice(audio_dev, 1);

    // Setup resampler for format conversion
    SwrContext* swr = swr_alloc();
    if (!swr) {
        std::cerr << "Could not allocate resampler\n";
        SDL_CloseAudioDevice(audio_dev);
        avcodec_free_context(&codec_ctx);
        avformat_close_input(&fmt_ctx);
        SDL_Quit();
        return -1;
    }
    
    AVChannelLayout out_ch_layout = AV_CHANNEL_LAYOUT_STEREO;
    if (nb_channels == 1) {
        out_ch_layout = AV_CHANNEL_LAYOUT_MONO;
    }
    
    av_opt_set_chlayout(swr, "in_chlayout", &codec_ctx->ch_layout, 0);
    av_opt_set_chlayout(swr, "out_chlayout", &out_ch_layout, 0);
    av_opt_set_int(swr, "in_sample_rate", codec_ctx->sample_rate, 0);
    av_opt_set_int(swr, "out_sample_rate", codec_ctx->sample_rate, 0);
    av_opt_set_sample_fmt(swr, "in_sample_fmt", codec_ctx->sample_fmt, 0);
    av_opt_set_sample_fmt(swr, "out_sample_fmt", AV_SAMPLE_FMT_S16, 0);
    
    if (swr_init(swr) < 0) {
        std::cerr << "Could not initialize resampler\n";
        swr_free(&swr);
        SDL_CloseAudioDevice(audio_dev);
        avcodec_free_context(&codec_ctx);
        avformat_close_input(&fmt_ctx);
        SDL_Quit();
        return -1;
    }

    AVPacket* pkt = av_packet_alloc();
    AVFrame* frame = av_frame_alloc();
    
    if (!pkt || !frame) {
        std::cerr << "Could not allocate packet or frame\n";
        if (pkt) av_packet_free(&pkt);
        if (frame) av_frame_free(&frame);
        swr_free(&swr);
        SDL_CloseAudioDevice(audio_dev);
        avcodec_free_context(&codec_ctx);
        avformat_close_input(&fmt_ctx);
        SDL_Quit();
        return -1;
    }

    // Buffer thresholds
    const uint32_t MIN_BUFFER_SIZE = 8192;    // Start playback when we have this much
    const uint32_t MAX_BUFFER_SIZE = 65536;   // Don't exceed this buffer size
    const uint32_t TARGET_BUFFER_SIZE = 32768; // Try to maintain around this level
    
    bool audio_started = false;
    bool end_of_file = false;

    // Main decoding loop
    while (!end_of_file) {
        // Check current buffer level
        uint32_t queued_audio_size = SDL_GetQueuedAudioSize(audio_dev);
        
        // If buffer is getting full, wait a bit
        if (queued_audio_size > MAX_BUFFER_SIZE) {
            SDL_Delay(10);
            continue;
        }
        
        // Start audio playback once we have enough data
        if (!audio_started && queued_audio_size >= MIN_BUFFER_SIZE) {
            SDL_PauseAudioDevice(audio_dev, 0);
            audio_started = true;
            std::cout << "Audio playback started\n";
        }
        
        // Read next packet
        int ret = av_read_frame(fmt_ctx, pkt);
        if (ret < 0) {
            if (ret == AVERROR_EOF) {
                // Send flush packet to decoder
                avcodec_send_packet(codec_ctx, nullptr);
                end_of_file = true;
            } else {
                std::cerr << "Error reading frame\n";
                break;
            }
        }
        
        // Skip non-audio packets
        if (!end_of_file && pkt->stream_index != audio_stream_index) {
            av_packet_unref(pkt);
            continue;
        }
        
        // Send packet to decoder
        if (!end_of_file) {
            ret = avcodec_send_packet(codec_ctx, pkt);
            if (ret < 0 && ret != AVERROR(EAGAIN)) {
                std::cerr << "Error sending packet to decoder\n";
                av_packet_unref(pkt);
                continue;
            }
        }
        
        // Receive all available frames from decoder
        while (true) {
            ret = avcodec_receive_frame(codec_ctx, frame);
            if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                break;
            }
            if (ret < 0) {
                std::cerr << "Error receiving frame from decoder\n";
                break;
            }
            
            // Calculate output buffer size
            int out_samples = swr_get_out_samples(swr, frame->nb_samples);
            if (out_samples <= 0) {
                continue;
            }
            
            // Allocate output buffer
            uint8_t* out_buffer[2] = {nullptr};
            int out_linesize;
            ret = av_samples_alloc(out_buffer, &out_linesize, nb_channels,
                                  out_samples, AV_SAMPLE_FMT_S16, 0);
            if (ret < 0) {
                std::cerr << "Could not allocate output buffer\n";
                continue;
            }
            
            // Convert audio format
            int converted_samples = swr_convert(swr, out_buffer, out_samples,
                                               (const uint8_t**)frame->data, frame->nb_samples);
            
            if (converted_samples > 0) {
                int audio_bytes = converted_samples * nb_channels * sizeof(int16_t);
                
                // Queue the audio data
                if (SDL_QueueAudio(audio_dev, out_buffer[0], audio_bytes) < 0) {
                    std::cerr << "SDL_QueueAudio error: " << SDL_GetError() << "\n";
                }
            }
            
            // Free the output buffer
            av_freep(&out_buffer[0]);
        }
        
        if (!end_of_file) {
            av_packet_unref(pkt);
        }
    }

    // Flush the resampler
    std::cout << "Flushing resampler...\n";
    while (true) {
        uint8_t* out_buffer[2] = {nullptr};
        int out_samples = 4096;
        int out_linesize;
        
        av_samples_alloc(out_buffer, &out_linesize, nb_channels,
                        out_samples, AV_SAMPLE_FMT_S16, 0);
        
        int converted_samples = swr_convert(swr, out_buffer, out_samples, nullptr, 0);
        
        if (converted_samples <= 0) {
            av_freep(&out_buffer[0]);
            break;
        }
        
        int audio_bytes = converted_samples * nb_channels * sizeof(int16_t);
        SDL_QueueAudio(audio_dev, out_buffer[0], audio_bytes);
        av_freep(&out_buffer[0]);
    }

    // Wait for all queued audio to finish playing
    std::cout << "Waiting for playback to complete...\n";
    while (SDL_GetQueuedAudioSize(audio_dev) > 0) {
        SDL_Delay(100);
    }
    
    std::cout << "Playback complete\n";

    // Cleanup
    SDL_CloseAudioDevice(audio_dev);
    SDL_Quit();
    av_frame_free(&frame);
    av_packet_free(&pkt);
    swr_free(&swr);
    avcodec_free_context(&codec_ctx);
    avformat_close_input(&fmt_ctx);

    return 0;
}