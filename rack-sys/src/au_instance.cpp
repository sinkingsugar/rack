#include "rack_au.h"
#include <AudioToolbox/AudioToolbox.h>
#include <CoreFoundation/CoreFoundation.h>
#include <cstring>
#include <cstdio>  // for sscanf
#include <climits> // for INT_MAX
#include <new>     // for std::align_val_t

#ifdef __ARM_NEON
#include <arm_neon.h>
#endif

#if defined(__x86_64__) || defined(_M_X64)
#include <emmintrin.h>  // SSE2
#endif

#include <mutex>

// Global mutex to serialize AudioUnit cleanup operations
// AudioUnitUninitialize/AudioComponentInstanceDispose are not fully thread-safe
static std::mutex g_audio_unit_cleanup_mutex;

// Internal plugin state
struct RackAUPlugin {
    AudioComponentInstance audio_unit;
    bool initialized;
    double sample_rate;
    uint32_t max_block_size;
    char unique_id[64];

    // Audio buffers for processing
    AudioBufferList* input_buffer_list;
    AudioBufferList* output_buffer_list;
    // Pointer to current input for render callback
    // Thread safety: AudioUnitRender is synchronous - the callback executes
    // on the calling thread before AudioUnitRender returns.
    // IMPORTANT: This means process() is NOT safe for concurrent calls from
    // multiple threads (race condition on current_input). Plugin instances
    // are Send but NOT Sync - must not be shared across threads during processing.
    const float* current_input;

    // Sample position tracking for AudioTimeStamp
    int64_t sample_position;

    // Parameter cache - populated during initialization to avoid redundant API calls
    AudioUnitParameterID* parameter_ids;
    AudioUnitParameterInfo* parameter_info;  // Cached parameter info for performance
    UInt32 parameter_count;
};

// ============================================================================
// Plugin Instance Implementation
// ============================================================================

// Helper function to convert AudioUnitParameterUnit enum to human-readable string
static const char* parameter_unit_to_string(AudioUnitParameterUnit unit) {
    switch (unit) {
        case kAudioUnitParameterUnit_Generic: return "";
        case kAudioUnitParameterUnit_Indexed: return "indexed";
        case kAudioUnitParameterUnit_Boolean: return "on/off";
        case kAudioUnitParameterUnit_Percent: return "%";
        case kAudioUnitParameterUnit_Seconds: return "s";
        case kAudioUnitParameterUnit_SampleFrames: return "samples";
        case kAudioUnitParameterUnit_Phase: return "°";
        case kAudioUnitParameterUnit_Rate: return "rate";
        case kAudioUnitParameterUnit_Hertz: return "Hz";
        case kAudioUnitParameterUnit_Cents: return "cents";
        case kAudioUnitParameterUnit_RelativeSemiTones: return "semitones";
        case kAudioUnitParameterUnit_MIDINoteNumber: return "note";
        case kAudioUnitParameterUnit_MIDIController: return "CC";
        case kAudioUnitParameterUnit_Decibels: return "dB";
        case kAudioUnitParameterUnit_LinearGain: return "gain";
        case kAudioUnitParameterUnit_Degrees: return "°";
        case kAudioUnitParameterUnit_EqualPowerCrossfade: return "xfade";
        case kAudioUnitParameterUnit_MixerFaderCurve1: return "fader";
        case kAudioUnitParameterUnit_Pan: return "pan";
        case kAudioUnitParameterUnit_Meters: return "m";
        case kAudioUnitParameterUnit_AbsoluteCents: return "cents";
        case kAudioUnitParameterUnit_Octaves: return "octaves";
        case kAudioUnitParameterUnit_BPM: return "BPM";
        case kAudioUnitParameterUnit_Beats: return "beats";
        case kAudioUnitParameterUnit_Milliseconds: return "ms";
        case kAudioUnitParameterUnit_Ratio: return "ratio";
        case kAudioUnitParameterUnit_CustomUnit: return "custom";
        default: return "";
    }
}

// Render callback: provides input audio to the AudioUnit
static OSStatus input_render_callback(
    void* inRefCon,
    AudioUnitRenderActionFlags* ioActionFlags,
    const AudioTimeStamp* inTimeStamp,
    UInt32 inBusNumber,
    UInt32 inNumberFrames,
    AudioBufferList* ioData
) {
    RackAUPlugin* plugin = static_cast<RackAUPlugin*>(inRefCon);

    if (!plugin || !plugin->current_input || !ioData) {
        *ioActionFlags |= kAudioUnitRenderAction_OutputIsSilence;
        return noErr;
    }

    // Bounds check: prevent buffer overrun if plugin requests more frames than allocated
    if (inNumberFrames > plugin->max_block_size) {
        *ioActionFlags |= kAudioUnitRenderAction_OutputIsSilence;
        return kAudioUnitErr_TooManyFramesToProcess;
    }

    // Copy interleaved input to non-interleaved AudioBufferList
    // NOTE: Currently hardcoded to stereo (2 channels) - mono/surround not yet supported
    const UInt32 required_bytes = inNumberFrames * sizeof(float);
    if (ioData->mNumberBuffers >= 2 &&
        ioData->mBuffers[0].mData &&
        ioData->mBuffers[1].mData &&
        ioData->mBuffers[0].mDataByteSize >= required_bytes &&
        ioData->mBuffers[1].mDataByteSize >= required_bytes) {

        float* left_out = static_cast<float*>(ioData->mBuffers[0].mData);
        float* right_out = static_cast<float*>(ioData->mBuffers[1].mData);
        const float* interleaved = plugin->current_input;

#ifdef __ARM_NEON
        // ARM NEON: SIMD-optimized deinterleaving, process 4 frames at a time
        UInt32 i = 0;
        UInt32 simd_frames = (inNumberFrames / 4) * 4;
        for (; i < simd_frames; i += 4) {
            float32x4x2_t stereo = vld2q_f32(&interleaved[i * 2]);
            vst1q_f32(&left_out[i], stereo.val[0]);
            vst1q_f32(&right_out[i], stereo.val[1]);
        }
        // Handle remaining frames (scalar fallback)
        for (; i < inNumberFrames; i++) {
            left_out[i] = interleaved[i * 2];
            right_out[i] = interleaved[i * 2 + 1];
        }
#elif defined(__x86_64__) || defined(_M_X64)
        // x86_64 SSE2: SIMD-optimized deinterleaving, process 4 frames at a time
        UInt32 i = 0;
        UInt32 simd_frames = (inNumberFrames / 4) * 4;
        for (; i < simd_frames; i += 4) {
            // Loads from Rust's aligned AudioBuffer (use unaligned for extra safety)
            __m128 pair0 = _mm_loadu_ps(&interleaved[i * 2]);      // L0 R0 L1 R1
            __m128 pair1 = _mm_loadu_ps(&interleaved[i * 2 + 4]);  // L2 R2 L3 R3

            // Shuffle to extract left: L0 L1 L2 L3
            __m128 left = _mm_shuffle_ps(pair0, pair1, _MM_SHUFFLE(2, 0, 2, 0));
            // Shuffle to extract right: R0 R1 R2 R3
            __m128 right = _mm_shuffle_ps(pair0, pair1, _MM_SHUFFLE(3, 1, 3, 1));

            // Unaligned stores to AudioUnit-provided buffers (alignment not guaranteed)
            _mm_storeu_ps(&left_out[i], left);
            _mm_storeu_ps(&right_out[i], right);
        }
        // Handle remaining frames (scalar fallback)
        for (; i < inNumberFrames; i++) {
            left_out[i] = interleaved[i * 2];
            right_out[i] = interleaved[i * 2 + 1];
        }
#else
        // Scalar fallback for other platforms
        for (UInt32 i = 0; i < inNumberFrames; i++) {
            left_out[i] = interleaved[i * 2];
            right_out[i] = interleaved[i * 2 + 1];
        }
#endif
    } else {
        // Buffer validation failed - return silence
        *ioActionFlags |= kAudioUnitRenderAction_OutputIsSilence;
    }

    return noErr;
}

// Parse unique_id format: "type-subtype-manufacturer" (all hex)
// Example: "61756678-64796e78-4170706c" (aufx-dynx-Appl)
static bool parse_unique_id(const char* unique_id, AudioComponentDescription* desc) {
    if (!unique_id || !desc) {
        return false;
    }

    unsigned int type = 0, subtype = 0, manufacturer = 0;
    int matched = sscanf(unique_id, "%x-%x-%x", &type, &subtype, &manufacturer);

    if (matched != 3) {
        return false;
    }

    desc->componentType = type;
    desc->componentSubType = subtype;
    desc->componentManufacturer = manufacturer;
    desc->componentFlags = 0;
    desc->componentFlagsMask = 0;

    return true;
}

RackAUPlugin* rack_au_plugin_new(const char* unique_id) {
    if (!unique_id) {
        return nullptr;
    }

    RackAUPlugin* plugin = new RackAUPlugin();
    plugin->audio_unit = nullptr;
    plugin->initialized = false;
    plugin->sample_rate = 0.0;
    plugin->max_block_size = 0;
    plugin->input_buffer_list = nullptr;
    plugin->output_buffer_list = nullptr;
    plugin->current_input = nullptr;
    plugin->sample_position = 0;
    plugin->parameter_ids = nullptr;
    plugin->parameter_info = nullptr;
    plugin->parameter_count = 0;
    strncpy(plugin->unique_id, unique_id, sizeof(plugin->unique_id) - 1);
    plugin->unique_id[sizeof(plugin->unique_id) - 1] = '\0';

    // Parse unique_id to get component description
    AudioComponentDescription desc;
    if (!parse_unique_id(unique_id, &desc)) {
        delete plugin;
        return nullptr;
    }

    // Find the AudioComponent
    AudioComponent component = AudioComponentFindNext(nullptr, &desc);
    if (!component) {
        delete plugin;
        return nullptr;
    }

    // Create the AudioComponentInstance
    // Serialize AudioComponent operations to avoid crashes in Apple's framework
    OSStatus status;
    {
        std::lock_guard<std::mutex> lock(g_audio_unit_cleanup_mutex);
        status = AudioComponentInstanceNew(component, &plugin->audio_unit);
    }
    if (status != noErr || !plugin->audio_unit) {
        delete plugin;
        return nullptr;
    }

    return plugin;
}

void rack_au_plugin_free(RackAUPlugin* plugin) {
    if (!plugin) {
        return;
    }

    if (plugin->audio_unit) {
        // Serialize AudioUnit cleanup to avoid crashes in Apple's framework
        // when multiple instances are being disposed concurrently
        std::lock_guard<std::mutex> lock(g_audio_unit_cleanup_mutex);
        AudioUnitUninitialize(plugin->audio_unit);
        AudioComponentInstanceDispose(plugin->audio_unit);
    }

    // Free audio buffers - free ALL channels, not just buffer[0]
    if (plugin->input_buffer_list) {
        for (UInt32 i = 0; i < plugin->input_buffer_list->mNumberBuffers; i++) {
            if (plugin->input_buffer_list->mBuffers[i].mData) {
                ::operator delete(plugin->input_buffer_list->mBuffers[i].mData, std::align_val_t{16});
            }
        }
        free(plugin->input_buffer_list);
    }

    if (plugin->output_buffer_list) {
        for (UInt32 i = 0; i < plugin->output_buffer_list->mNumberBuffers; i++) {
            if (plugin->output_buffer_list->mBuffers[i].mData) {
                ::operator delete(plugin->output_buffer_list->mBuffers[i].mData, std::align_val_t{16});
            }
        }
        free(plugin->output_buffer_list);
    }

    // Free parameter cache
    if (plugin->parameter_ids) {
        free(plugin->parameter_ids);
    }
    if (plugin->parameter_info) {
        free(plugin->parameter_info);
    }

    delete plugin;
}

int rack_au_plugin_initialize(RackAUPlugin* plugin, double sample_rate, uint32_t max_block_size) {
    if (!plugin) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    if (!plugin->audio_unit) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (plugin->initialized) {
        return RACK_AU_OK;  // Already initialized
    }

    plugin->sample_rate = sample_rate;
    plugin->max_block_size = max_block_size;

    // Set up audio stream format (stereo, interleaved, 32-bit float)
    AudioStreamBasicDescription format;
    memset(&format, 0, sizeof(format));
    format.mSampleRate = sample_rate;
    format.mFormatID = kAudioFormatLinearPCM;
    format.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked;
    format.mBitsPerChannel = 32;
    format.mChannelsPerFrame = 2;  // Stereo
    format.mFramesPerPacket = 1;
    format.mBytesPerFrame = sizeof(float) * 2;  // 2 channels interleaved
    format.mBytesPerPacket = format.mBytesPerFrame * format.mFramesPerPacket;

    // Try to set the format on both input and output scopes
    // Different plugin types support different scopes, so we try both
    OSStatus status_input = AudioUnitSetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_StreamFormat,
        kAudioUnitScope_Input,
        0,  // Element 0
        &format,
        sizeof(format)
    );

    OSStatus status_output = AudioUnitSetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_StreamFormat,
        kAudioUnitScope_Output,
        0,  // Element 0
        &format,
        sizeof(format)
    );

    // At least one scope should succeed for the plugin to be usable
    // Some plugins (instruments) don't have input, others don't have output configured
    // We'll be permissive here and just warn if both fail
    if (status_input != noErr && status_output != noErr) {
        // Both failed - this might be a problem, but let's try to continue
        // Some plugins might not need explicit format setting
    }

    // Set maximum frames per slice
    UInt32 max_frames = max_block_size;
    OSStatus status = AudioUnitSetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_MaximumFramesPerSlice,
        kAudioUnitScope_Global,
        0,
        &max_frames,
        sizeof(max_frames)
    );

    if (status != noErr) {
        // MaximumFramesPerSlice might not be supported by all plugins, continue anyway
    }

    // Allocate audio buffers for non-interleaved format
    // NOTE: Phase 4 limitation - hardcoded to stereo (2 channels)
    // TODO (Phase 6+): Query plugin's actual channel configuration and support mono/surround
    // Input buffer (for providing audio to effect plugins)
    size_t buffer_list_size = offsetof(AudioBufferList, mBuffers[0]) + (sizeof(AudioBuffer) * 2);
    plugin->input_buffer_list = static_cast<AudioBufferList*>(malloc(buffer_list_size));
    if (!plugin->input_buffer_list) {
        return RACK_AU_ERROR_GENERIC;  // Memory allocation failed
    }
    plugin->input_buffer_list->mNumberBuffers = 2;  // Stereo only

    size_t buffer_size = max_block_size * sizeof(float);
    for (UInt32 i = 0; i < 2; i++) {
        plugin->input_buffer_list->mBuffers[i].mNumberChannels = 1;
        plugin->input_buffer_list->mBuffers[i].mDataByteSize = buffer_size;
        // Allocate 16-byte aligned buffer for SIMD operations
        plugin->input_buffer_list->mBuffers[i].mData = ::operator new(buffer_size, std::align_val_t{16});
        if (!plugin->input_buffer_list->mBuffers[i].mData) {
            // Clean up partially allocated buffers
            for (UInt32 j = 0; j < i; j++) {
                ::operator delete(plugin->input_buffer_list->mBuffers[j].mData, std::align_val_t{16});
            }
            free(plugin->input_buffer_list);
            plugin->input_buffer_list = nullptr;
            return RACK_AU_ERROR_GENERIC;  // Memory allocation failed
        }
    }

    // Output buffer (for receiving audio from the plugin)
    plugin->output_buffer_list = static_cast<AudioBufferList*>(malloc(buffer_list_size));
    if (!plugin->output_buffer_list) {
        // Clean up input buffers
        for (UInt32 i = 0; i < 2; i++) {
            ::operator delete(plugin->input_buffer_list->mBuffers[i].mData, std::align_val_t{16});
        }
        free(plugin->input_buffer_list);
        plugin->input_buffer_list = nullptr;
        return RACK_AU_ERROR_GENERIC;  // Memory allocation failed
    }
    plugin->output_buffer_list->mNumberBuffers = 2;

    for (UInt32 i = 0; i < 2; i++) {
        plugin->output_buffer_list->mBuffers[i].mNumberChannels = 1;
        plugin->output_buffer_list->mBuffers[i].mDataByteSize = buffer_size;
        // Allocate 16-byte aligned buffer for SIMD operations
        plugin->output_buffer_list->mBuffers[i].mData = ::operator new(buffer_size, std::align_val_t{16});
        if (!plugin->output_buffer_list->mBuffers[i].mData) {
            // Clean up all allocated buffers
            for (UInt32 j = 0; j < i; j++) {
                ::operator delete(plugin->output_buffer_list->mBuffers[j].mData, std::align_val_t{16});
            }
            free(plugin->output_buffer_list);
            for (UInt32 j = 0; j < 2; j++) {
                ::operator delete(plugin->input_buffer_list->mBuffers[j].mData, std::align_val_t{16});
            }
            free(plugin->input_buffer_list);
            plugin->input_buffer_list = nullptr;
            plugin->output_buffer_list = nullptr;
            // Clean up AudioUnit instance to prevent leak
            AudioComponentInstanceDispose(plugin->audio_unit);
            plugin->audio_unit = nullptr;
            return RACK_AU_ERROR_GENERIC;  // Memory allocation failed
        }
    }

    // Set up input render callback (for effect plugins)
    AURenderCallbackStruct callback;
    callback.inputProc = input_render_callback;
    callback.inputProcRefCon = plugin;

    status = AudioUnitSetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_SetRenderCallback,
        kAudioUnitScope_Input,
        0,
        &callback,
        sizeof(callback)
    );

    // This may fail for instruments (no input), which is okay
    // We don't return error here

    // Initialize the AudioUnit
    // Serialize AudioUnit initialization to avoid crashes in Apple's framework
    {
        std::lock_guard<std::mutex> lock(g_audio_unit_cleanup_mutex);
        status = AudioUnitInitialize(plugin->audio_unit);
    }
    if (status != noErr) {
        // Clean up buffers on failure
        if (plugin->input_buffer_list) {
            for (UInt32 i = 0; i < 2; i++) {
                ::operator delete(plugin->input_buffer_list->mBuffers[i].mData, std::align_val_t{16});
            }
            free(plugin->input_buffer_list);
            plugin->input_buffer_list = nullptr;
        }
        if (plugin->output_buffer_list) {
            for (UInt32 i = 0; i < 2; i++) {
                ::operator delete(plugin->output_buffer_list->mBuffers[i].mData, std::align_val_t{16});
            }
            free(plugin->output_buffer_list);
            plugin->output_buffer_list = nullptr;
        }
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    // Query parameter list (may fail for plugins without parameters)
    UInt32 data_size = 0;
    status = AudioUnitGetPropertyInfo(
        plugin->audio_unit,
        kAudioUnitProperty_ParameterList,
        kAudioUnitScope_Global,
        0,
        &data_size,
        nullptr
    );

    if (status == noErr && data_size > 0) {
        plugin->parameter_count = data_size / sizeof(AudioUnitParameterID);
        plugin->parameter_ids = static_cast<AudioUnitParameterID*>(malloc(data_size));

        if (plugin->parameter_ids) {
            status = AudioUnitGetProperty(
                plugin->audio_unit,
                kAudioUnitProperty_ParameterList,
                kAudioUnitScope_Global,
                0,
                plugin->parameter_ids,
                &data_size
            );

            if (status != noErr) {
                // Failed to get parameter list, clean up
                free(plugin->parameter_ids);
                plugin->parameter_ids = nullptr;
                plugin->parameter_count = 0;
            } else {
                // Cache parameter info for all parameters to avoid redundant API calls
                // during get/set operations (critical for real-time automation)
                plugin->parameter_info = static_cast<AudioUnitParameterInfo*>(
                    malloc(plugin->parameter_count * sizeof(AudioUnitParameterInfo))
                );

                if (plugin->parameter_info) {
                    // Query info for each parameter
                    for (UInt32 i = 0; i < plugin->parameter_count; i++) {
                        UInt32 info_size = sizeof(AudioUnitParameterInfo);
                        OSStatus info_status = AudioUnitGetProperty(
                            plugin->audio_unit,
                            kAudioUnitProperty_ParameterInfo,
                            kAudioUnitScope_Global,
                            plugin->parameter_ids[i],
                            &plugin->parameter_info[i],
                            &info_size
                        );

                        if (info_status != noErr) {
                            // If we can't get info for any parameter, invalidate the entire cache
                            // to fall back to per-call queries (safer than partial cache)
                            free(plugin->parameter_info);
                            plugin->parameter_info = nullptr;
                            // NOTE: parameter_ids is intentionally kept here (not a leak)
                            // - Still needed for parameter enumeration and get/set operations
                            // - Get/set/info functions have fallback code that queries on-demand when cache is NULL
                            // - Will be freed in rack_au_plugin_free() during cleanup
                            break;
                        }
                    }
                }
            }
        } else {
            plugin->parameter_count = 0;
        }
    }

    plugin->initialized = true;
    return RACK_AU_OK;
}

int rack_au_plugin_is_initialized(RackAUPlugin* plugin) {
    return plugin && plugin->initialized ? 1 : 0;
}

int rack_au_plugin_process(RackAUPlugin* plugin, const float* input, float* output, uint32_t frames) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (!input || !output || frames == 0) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    if (frames > plugin->max_block_size) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // Set current input for render callback
    plugin->current_input = input;

    // Set up AudioTimeStamp with running sample position
    AudioTimeStamp timestamp;
    memset(&timestamp, 0, sizeof(timestamp));
    timestamp.mFlags = kAudioTimeStampSampleTimeValid;
    timestamp.mSampleTime = plugin->sample_position;

    // Render audio from the AudioUnit
    AudioUnitRenderActionFlags flags = 0;
    OSStatus status = AudioUnitRender(
        plugin->audio_unit,
        &flags,
        &timestamp,
        0,  // Output bus
        frames,
        plugin->output_buffer_list
    );

    plugin->current_input = nullptr;

    if (status != noErr) {
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    // Convert non-interleaved output to interleaved stereo
    // Verify buffers are valid before dereferencing
    if (plugin->output_buffer_list->mNumberBuffers >= 2 &&
        plugin->output_buffer_list->mBuffers[0].mData &&
        plugin->output_buffer_list->mBuffers[1].mData &&
        plugin->output_buffer_list->mBuffers[0].mDataByteSize >= frames * sizeof(float) &&
        plugin->output_buffer_list->mBuffers[1].mDataByteSize >= frames * sizeof(float)) {

        const float* left_in = static_cast<const float*>(plugin->output_buffer_list->mBuffers[0].mData);
        const float* right_in = static_cast<const float*>(plugin->output_buffer_list->mBuffers[1].mData);

#ifdef __ARM_NEON
        // ARM NEON: SIMD-optimized interleaving, process 4 frames at a time
        uint32_t i = 0;
        uint32_t simd_frames = (frames / 4) * 4;
        for (; i < simd_frames; i += 4) {
            float32x4x2_t stereo;
            stereo.val[0] = vld1q_f32(&left_in[i]);
            stereo.val[1] = vld1q_f32(&right_in[i]);
            vst2q_f32(&output[i * 2], stereo);
        }
        // Handle remaining frames (scalar fallback)
        for (; i < frames; i++) {
            output[i * 2] = left_in[i];
            output[i * 2 + 1] = right_in[i];
        }
#elif defined(__x86_64__) || defined(_M_X64)
        // x86_64 SSE2: SIMD-optimized interleaving, process 4 frames at a time
        uint32_t i = 0;
        uint32_t simd_frames = (frames / 4) * 4;
        for (; i < simd_frames; i += 4) {
            // Unaligned loads from AudioUnit buffers (defensive - we allocate them aligned, but be safe)
            __m128 left = _mm_loadu_ps(&left_in[i]);   // L0 L1 L2 L3
            __m128 right = _mm_loadu_ps(&right_in[i]); // R0 R1 R2 R3

            // Interleave low half: L0 R0 L1 R1
            __m128 low = _mm_unpacklo_ps(left, right);
            // Interleave high half: L2 R2 L3 R3
            __m128 high = _mm_unpackhi_ps(left, right);

            // Stores to Rust's 16-byte aligned AudioBuffer
            // Note: Mathematically guaranteed to be 16-byte aligned due to i being multiple of 4,
            // but using unaligned stores for extra safety with negligible performance cost
            _mm_storeu_ps(&output[i * 2], low);
            _mm_storeu_ps(&output[i * 2 + 4], high);
        }
        // Handle remaining frames (scalar fallback)
        for (; i < frames; i++) {
            output[i * 2] = left_in[i];
            output[i * 2 + 1] = right_in[i];
        }
#else
        // Scalar fallback for other platforms
        for (uint32_t i = 0; i < frames; i++) {
            output[i * 2] = left_in[i];
            output[i * 2 + 1] = right_in[i];
        }
#endif
    } else {
        // Buffer validation failed - return silence
        memset(output, 0, frames * 2 * sizeof(float));
    }

    // Update sample position for next call
    // Note: int64_t will overflow after ~6 million years at 48kHz
    // This is not a practical concern, but some AudioUnits may expect wrapping behavior
    plugin->sample_position += frames;

    return RACK_AU_OK;
}

int rack_au_plugin_parameter_count(RackAUPlugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return 0;
    }

    return static_cast<int>(plugin->parameter_count);
}

int rack_au_plugin_get_parameter(RackAUPlugin* plugin, uint32_t index, float* value) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (!value) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    if (index >= plugin->parameter_count) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    AudioUnitParameterID param_id = plugin->parameter_ids[index];

    // Get parameter info for min/max values (needed for normalization)
    // Use cached info if available (performance optimization for real-time use)
    AudioUnitParameterInfo param_info;

    if (plugin->parameter_info) {
        // Use cached parameter info (fast path)
        param_info = plugin->parameter_info[index];
    } else {
        // Fall back to querying parameter info (slow path - cache failed to initialize)
        UInt32 data_size = sizeof(param_info);
        OSStatus status = AudioUnitGetProperty(
            plugin->audio_unit,
            kAudioUnitProperty_ParameterInfo,
            kAudioUnitScope_Global,
            param_id,
            &param_info,
            &data_size
        );

        if (status != noErr) {
            return RACK_AU_ERROR_AUDIO_UNIT + status;
        }
    }

    // Get current parameter value
    AudioUnitParameterValue raw_value;
    OSStatus status = AudioUnitGetParameter(
        plugin->audio_unit,
        param_id,
        kAudioUnitScope_Global,
        0,
        &raw_value
    );

    if (status != noErr) {
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    // Normalize to 0.0-1.0 range
    float min_val = param_info.minValue;
    float max_val = param_info.maxValue;
    float range = max_val - min_val;

    // Validate parameter range (detect malformed AudioUnit parameter info)
    if (max_val < min_val) {
        // Invalid range - this shouldn't happen with well-formed AudioUnits
        // Return mid-range as safe fallback
        *value = 0.5f;
        return RACK_AU_OK;
    }

    // Use epsilon comparison for floating-point safety
    const float epsilon = 1e-7f;
    if (range > epsilon) {
        *value = (raw_value - min_val) / range;
        // Clamp to 0.0-1.0 in case of floating-point rounding errors
        if (*value < 0.0f) *value = 0.0f;
        if (*value > 1.0f) *value = 1.0f;
    } else {
        // Zero or near-zero range - return 0.0 (parameter has single value)
        *value = 0.0f;
    }

    return RACK_AU_OK;
}

int rack_au_plugin_set_parameter(RackAUPlugin* plugin, uint32_t index, float value) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (index >= plugin->parameter_count) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // Clamp normalized value to 0.0-1.0
    if (value < 0.0f) value = 0.0f;
    if (value > 1.0f) value = 1.0f;

    AudioUnitParameterID param_id = plugin->parameter_ids[index];

    // Get parameter info for min/max values (needed for denormalization)
    // Use cached info if available (performance optimization for real-time use)
    AudioUnitParameterInfo param_info;

    if (plugin->parameter_info) {
        // Use cached parameter info (fast path)
        param_info = plugin->parameter_info[index];
    } else {
        // Fall back to querying parameter info (slow path - cache failed to initialize)
        UInt32 data_size = sizeof(param_info);
        OSStatus status = AudioUnitGetProperty(
            plugin->audio_unit,
            kAudioUnitProperty_ParameterInfo,
            kAudioUnitScope_Global,
            param_id,
            &param_info,
            &data_size
        );

        if (status != noErr) {
            return RACK_AU_ERROR_AUDIO_UNIT + status;
        }
    }

    // Denormalize from 0.0-1.0 to actual parameter range
    float min_val = param_info.minValue;
    float max_val = param_info.maxValue;
    float raw_value = min_val + (value * (max_val - min_val));

    // Set parameter value (with 0 sample offset for immediate change)
    OSStatus status = AudioUnitSetParameter(
        plugin->audio_unit,
        param_id,
        kAudioUnitScope_Global,
        0,
        raw_value,
        0  // Sample offset (0 = immediate)
    );

    if (status != noErr) {
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    return RACK_AU_OK;
}

int rack_au_plugin_parameter_info(
    RackAUPlugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    float* min,
    float* max,
    float* default_value,
    char* unit,
    size_t unit_size
) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (!name || !min || !max || !default_value) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    if (index >= plugin->parameter_count) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    AudioUnitParameterID param_id = plugin->parameter_ids[index];

    // Get parameter info - use cached if available (performance optimization)
    AudioUnitParameterInfo param_info;

    if (plugin->parameter_info) {
        // Use cached parameter info (fast path)
        param_info = plugin->parameter_info[index];
    } else {
        // Fall back to querying parameter info (slow path - cache failed to initialize)
        UInt32 data_size = sizeof(param_info);
        OSStatus status = AudioUnitGetProperty(
            plugin->audio_unit,
            kAudioUnitProperty_ParameterInfo,
            kAudioUnitScope_Global,
            param_id,
            &param_info,
            &data_size
        );

        if (status != noErr) {
            return RACK_AU_ERROR_AUDIO_UNIT + status;
        }
    }

    // Extract parameter name
    // CFString memory note: cfNameString is owned by the AudioUnit - we copy it here
    // and don't need to CFRelease it (AudioUnit manages the lifecycle).
    // Reference: Audio Unit Programming Guide, "Getting Parameter Information"
    // https://developer.apple.com/library/archive/documentation/MusicAudio/Conceptual/AudioUnitProgrammingGuide/
    if (param_info.cfNameString) {
        CFStringGetCString(
            param_info.cfNameString,
            name,
            name_size,
            kCFStringEncodingUTF8
        );
    } else {
        // Fallback: use parameter ID as name
        snprintf(name, name_size, "Parameter %u", param_id);
    }

    // Extract parameter unit string (optional)
    if (unit && unit_size > 0) {
        const char* unit_str = parameter_unit_to_string(param_info.unit);
        strncpy(unit, unit_str, unit_size - 1);
        unit[unit_size - 1] = '\0';  // Ensure null termination
    }

    // Extract min/max/default values
    *min = param_info.minValue;
    *max = param_info.maxValue;
    *default_value = param_info.defaultValue;

    return RACK_AU_OK;
}

// ============================================================================
// Preset Management Implementation
// ============================================================================

int rack_au_plugin_get_preset_count(RackAUPlugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return 0;
    }

    // Query factory presets
    CFArrayRef presets = nullptr;
    UInt32 data_size = sizeof(presets);
    OSStatus status = AudioUnitGetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_FactoryPresets,
        kAudioUnitScope_Global,
        0,
        &presets,
        &data_size
    );

    if (status != noErr || !presets) {
        return 0;  // Plugin has no factory presets
    }

    CFIndex count = CFArrayGetCount(presets);
    // Note: presets CFArrayRef is not owned by us - don't CFRelease
    // Reference: Audio Unit Programming Guide, "Factory Presets"
    return static_cast<int>(count);
}

int rack_au_plugin_get_preset_info(
    RackAUPlugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    int32_t* preset_number
) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (!name || !preset_number) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // Query factory presets
    CFArrayRef presets = nullptr;
    UInt32 data_size = sizeof(presets);
    OSStatus status = AudioUnitGetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_FactoryPresets,
        kAudioUnitScope_Global,
        0,
        &presets,
        &data_size
    );

    if (status != noErr || !presets) {
        return RACK_AU_ERROR_NOT_FOUND;
    }

    CFIndex count = CFArrayGetCount(presets);
    if (index >= static_cast<uint32_t>(count)) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // Get preset at index
    // AUPreset is a struct: { presetNumber: SInt32, presetName: CFStringRef }
    const AUPreset* preset = static_cast<const AUPreset*>(
        CFArrayGetValueAtIndex(presets, index)
    );

    if (!preset) {
        return RACK_AU_ERROR_GENERIC;
    }

    // Extract preset number
    *preset_number = preset->presetNumber;

    // Extract preset name
    if (preset->presetName) {
        Boolean success = CFStringGetCString(
            preset->presetName,
            name,
            name_size,
            kCFStringEncodingUTF8
        );

        if (!success) {
            // Fallback: buffer too small or encoding failed
            snprintf(name, name_size, "Preset %d", preset->presetNumber);
        }
    } else {
        // Fallback: use preset number as name
        snprintf(name, name_size, "Preset %d", preset->presetNumber);
    }

    return RACK_AU_OK;
}

int rack_au_plugin_load_preset(RackAUPlugin* plugin, int32_t preset_number) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    // Create AUPreset struct
    AUPreset preset;
    preset.presetNumber = preset_number;
    preset.presetName = nullptr;  // AudioUnit will look up name by number

    // Set current preset
    OSStatus status = AudioUnitSetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_PresentPreset,
        kAudioUnitScope_Global,
        0,
        &preset,
        sizeof(preset)
    );

    if (status != noErr) {
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    return RACK_AU_OK;
}

int rack_au_plugin_get_state_size(RackAUPlugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return 0;
    }

    // Query ClassInfo (full plugin state as CFPropertyList)
    CFPropertyListRef class_info = nullptr;
    UInt32 data_size = sizeof(class_info);
    OSStatus status = AudioUnitGetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_ClassInfo,
        kAudioUnitScope_Global,
        0,
        &class_info,
        &data_size
    );

    if (status != noErr || !class_info) {
        return 0;
    }

    // Serialize to binary data to determine size
    CFDataRef data = CFPropertyListCreateData(
        kCFAllocatorDefault,
        class_info,
        kCFPropertyListBinaryFormat_v1_0,
        0,  // options
        nullptr  // error
    );

    CFRelease(class_info);  // We own class_info, must release

    if (!data) {
        return 0;
    }

    CFIndex size = CFDataGetLength(data);
    CFRelease(data);

    // Validate size fits in int (prevent overflow)
    if (size > INT_MAX || size < 0) {
        return 0;  // State too large or invalid
    }

    return static_cast<int>(size);
}

int rack_au_plugin_get_state(RackAUPlugin* plugin, uint8_t* data, size_t* size) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (!data || !size) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // Query ClassInfo (full plugin state)
    CFPropertyListRef class_info = nullptr;
    UInt32 data_size = sizeof(class_info);
    OSStatus status = AudioUnitGetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_ClassInfo,
        kAudioUnitScope_Global,
        0,
        &class_info,
        &data_size
    );

    if (status != noErr || !class_info) {
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    // Serialize to binary data
    CFDataRef cf_data = CFPropertyListCreateData(
        kCFAllocatorDefault,
        class_info,
        kCFPropertyListBinaryFormat_v1_0,
        0,  // options
        nullptr  // error
    );

    CFRelease(class_info);  // We own class_info, must release

    if (!cf_data) {
        return RACK_AU_ERROR_GENERIC;
    }

    // Copy data to output buffer
    CFIndex cf_size = CFDataGetLength(cf_data);
    if (static_cast<size_t>(cf_size) > *size) {
        CFRelease(cf_data);
        return RACK_AU_ERROR_GENERIC;  // Buffer too small
    }

    const UInt8* bytes = CFDataGetBytePtr(cf_data);
    memcpy(data, bytes, cf_size);
    *size = cf_size;

    CFRelease(cf_data);

    return RACK_AU_OK;
}

int rack_au_plugin_set_state(RackAUPlugin* plugin, const uint8_t* data, size_t size) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (!data || size == 0) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // Create CFData from input buffer
    CFDataRef cf_data = CFDataCreate(
        kCFAllocatorDefault,
        data,
        size
    );

    if (!cf_data) {
        return RACK_AU_ERROR_GENERIC;
    }

    // Deserialize CFPropertyList from binary data
    CFPropertyListRef class_info = CFPropertyListCreateWithData(
        kCFAllocatorDefault,
        cf_data,
        kCFPropertyListImmutable,
        nullptr,  // format (output)
        nullptr   // error
    );

    CFRelease(cf_data);

    if (!class_info) {
        return RACK_AU_ERROR_GENERIC;  // Failed to deserialize
    }

    // Note: We don't validate the property list type here because:
    // 1. CFPropertyListCreateWithData already validates the binary format
    // 2. AudioUnit's SetProperty will validate the structure and reject invalid data
    // 3. Some AudioUnits may use non-dictionary top-level types
    // Strict type checking was causing SIGBUS in CI with certain plugin states.

    // Restore plugin state
    OSStatus status = AudioUnitSetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_ClassInfo,
        kAudioUnitScope_Global,
        0,
        &class_info,
        sizeof(class_info)
    );

    CFRelease(class_info);  // We own class_info, must release

    if (status != noErr) {
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    return RACK_AU_OK;
}

// ============================================================================
// MIDI Implementation
// ============================================================================

int rack_au_plugin_send_midi(
    RackAUPlugin* plugin,
    const RackAUMidiEvent* events,
    uint32_t event_count
) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (!events && event_count > 0) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // Early return if no events to send
    if (event_count == 0) {
        return RACK_AU_OK;
    }

    // Send each MIDI event to the AudioUnit
    // Using MusicDeviceMIDIEvent for sample-accurate timing via event->sample_offset
    for (uint32_t i = 0; i < event_count; i++) {
        const RackAUMidiEvent* event = &events[i];

        uint8_t status;

        // System messages (0xF0-0xFF) don't use channels
        if (event->status >= 0xF0) {
            // System message - use status byte as-is
            status = event->status;
        } else {
            // Channel message - validate channel and combine
            if (event->channel > 15) {
                return RACK_AU_ERROR_INVALID_PARAM;
            }
            // Clear any channel bits from status (use upper nibble only), then combine with channel
            // Status byte upper nibble (0x90, 0x80, etc.) + channel lower nibble (0-15)
            status = (event->status & 0xF0) | (event->channel & 0x0F);
        }

        // Send MIDI event to AudioUnit
        // MusicDeviceMIDIEvent is the primary API for sending MIDI to instrument plugins
        // For effect plugins that don't implement this, the call will fail gracefully
        OSStatus result = MusicDeviceMIDIEvent(
            plugin->audio_unit,
            status,
            event->data1,
            event->data2,
            event->sample_offset  // Sample offset within buffer for sample-accurate timing
        );

        // Note: Some effect plugins don't support MIDI, so we check the error
        // and return it to let Rust handle the failure gracefully
        if (result != noErr) {
            return RACK_AU_ERROR_AUDIO_UNIT + result;
        }
    }

    return RACK_AU_OK;
}

// ============================================================================
// GUI Helper
// ============================================================================

// Helper function for GUI code to access AudioComponentInstance
// This allows au_gui.mm to get the audio_unit without accessing the opaque struct
extern "C" AudioComponentInstance rack_au_plugin_get_audio_unit(RackAUPlugin* plugin) {
    if (!plugin) {
        return NULL;
    }
    return plugin->audio_unit;
}
