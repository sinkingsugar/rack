#include "rack_au.h"
#include <AudioToolbox/AudioToolbox.h>
#include <CoreFoundation/CoreFoundation.h>
#include <cstring>
#include <cstdio>  // for sscanf
#include <climits> // for INT_MAX
#include <new>     // for std::align_val_t
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

    // Audio buffers for processing (planar format - one buffer per channel)
    AudioBufferList* input_buffer_list;
    AudioBufferList* output_buffer_list;

    // Channel configuration (queried from AudioUnit during initialize)
    uint32_t input_channels;
    uint32_t output_channels;

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
// Now works with planar data (no interleave/deinterleave conversion needed)
static OSStatus input_render_callback(
    void* inRefCon,
    AudioUnitRenderActionFlags* ioActionFlags,
    const AudioTimeStamp* inTimeStamp,
    UInt32 inBusNumber,
    UInt32 inNumberFrames,
    AudioBufferList* ioData
) {
    RackAUPlugin* plugin = static_cast<RackAUPlugin*>(inRefCon);

    if (!plugin || !plugin->input_buffer_list || !ioData) {
        *ioActionFlags |= kAudioUnitRenderAction_OutputIsSilence;
        return noErr;
    }

    // Bounds check: prevent buffer overrun if plugin requests more frames than allocated
    if (inNumberFrames > plugin->max_block_size) {
        *ioActionFlags |= kAudioUnitRenderAction_OutputIsSilence;
        return kAudioUnitErr_TooManyFramesToProcess;
    }

    // Copy planar input from our buffers to AudioUnit's buffers (planar → planar, no conversion!)
    UInt32 num_channels = ioData->mNumberBuffers < plugin->input_buffer_list->mNumberBuffers
                              ? ioData->mNumberBuffers
                              : plugin->input_buffer_list->mNumberBuffers;

    const UInt32 required_bytes = inNumberFrames * sizeof(float);
    for (UInt32 ch = 0; ch < num_channels; ch++) {
        if (ioData->mBuffers[ch].mData &&
            ioData->mBuffers[ch].mDataByteSize >= required_bytes &&
            plugin->input_buffer_list->mBuffers[ch].mData) {

            // Safety: This memcpy is safe because:
            // 1. inNumberFrames validated at lines 95-98 to not exceed max_block_size
            // 2. mData points to caller's buffer (validated in Rust to have ≥max_block_size frames)
            // 3. Channel count validated in Rust process() before reaching here
            const float* src = static_cast<const float*>(plugin->input_buffer_list->mBuffers[ch].mData);
            float* dest = static_cast<float*>(ioData->mBuffers[ch].mData);
            memcpy(dest, src, required_bytes);
        }
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
    plugin->input_channels = 0;
    plugin->output_channels = 0;
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

    // Free AudioBufferList structures (zero-copy: we don't own the buffer memory)
    if (plugin->input_buffer_list) {
        free(plugin->input_buffer_list);
    }

    if (plugin->output_buffer_list) {
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

    // Default to stereo for compatibility - query actual config after initialization
    uint32_t channels = 2;

    // Set up audio stream format (planar/non-interleaved, 32-bit float)
    AudioStreamBasicDescription format;
    memset(&format, 0, sizeof(format));
    format.mSampleRate = sample_rate;
    format.mFormatID = kAudioFormatLinearPCM;
    format.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked | kAudioFormatFlagIsNonInterleaved;
    format.mBitsPerChannel = 32;
    format.mChannelsPerFrame = channels;
    format.mFramesPerPacket = 1;
    format.mBytesPerFrame = sizeof(float);  // Per channel (non-interleaved)
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

    // Query actual channel configuration after setting format
    AudioStreamBasicDescription actual_input_format;
    UInt32 size_of_format = sizeof(actual_input_format);
    OSStatus query_status = AudioUnitGetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_StreamFormat,
        kAudioUnitScope_Input,
        0,
        &actual_input_format,
        &size_of_format
    );
    uint32_t input_channels = (query_status == noErr) ? actual_input_format.mChannelsPerFrame : channels;

    AudioStreamBasicDescription actual_output_format;
    query_status = AudioUnitGetProperty(
        plugin->audio_unit,
        kAudioUnitProperty_StreamFormat,
        kAudioUnitScope_Output,
        0,
        &actual_output_format,
        &size_of_format
    );
    uint32_t output_channels = (query_status == noErr) ? actual_output_format.mChannelsPerFrame : channels;

    // Store channel configuration
    plugin->input_channels = input_channels;
    plugin->output_channels = output_channels;

    // Allocate audio buffers for planar/non-interleaved format
    // Input buffer (for providing audio to effect plugins)
    size_t buffer_list_size = offsetof(AudioBufferList, mBuffers[0]) + (sizeof(AudioBuffer) * input_channels);
    plugin->input_buffer_list = static_cast<AudioBufferList*>(malloc(buffer_list_size));
    if (!plugin->input_buffer_list) {
        return RACK_AU_ERROR_GENERIC;  // Memory allocation failed
    }
    plugin->input_buffer_list->mNumberBuffers = input_channels;

    // Zero-copy approach: we'll point mData at caller's buffers in process()
    for (UInt32 i = 0; i < input_channels; i++) {
        plugin->input_buffer_list->mBuffers[i].mNumberChannels = 1;
        plugin->input_buffer_list->mBuffers[i].mDataByteSize = 0;  // Updated in process()
        plugin->input_buffer_list->mBuffers[i].mData = nullptr;    // Updated in process()
    }

    // Output buffer (for receiving audio from the plugin)
    size_t output_buffer_list_size = offsetof(AudioBufferList, mBuffers[0]) + (sizeof(AudioBuffer) * output_channels);
    plugin->output_buffer_list = static_cast<AudioBufferList*>(malloc(output_buffer_list_size));
    if (!plugin->output_buffer_list) {
        // Clean up input buffer list (zero-copy: no mData to free)
        free(plugin->input_buffer_list);
        plugin->input_buffer_list = nullptr;
        return RACK_AU_ERROR_GENERIC;  // Memory allocation failed
    }
    plugin->output_buffer_list->mNumberBuffers = output_channels;

    // Zero-copy approach: we'll point mData at caller's buffers in process()
    for (UInt32 i = 0; i < output_channels; i++) {
        plugin->output_buffer_list->mBuffers[i].mNumberChannels = 1;
        plugin->output_buffer_list->mBuffers[i].mDataByteSize = 0;  // Updated in process()
        plugin->output_buffer_list->mBuffers[i].mData = nullptr;    // Updated in process()
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
        // Clean up buffer lists on failure (zero-copy: no mData to free)
        if (plugin->input_buffer_list) {
            free(plugin->input_buffer_list);
            plugin->input_buffer_list = nullptr;
        }
        if (plugin->output_buffer_list) {
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

int rack_au_plugin_reset(RackAUPlugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    // Call AudioUnitReset to clear all internal state
    OSStatus status = AudioUnitReset(plugin->audio_unit, kAudioUnitScope_Global, 0);

    if (status != noErr) {
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    return RACK_AU_OK;
}

int rack_au_plugin_process(
    RackAUPlugin* plugin,
    const float* const* inputs,
    uint32_t num_input_channels,
    float* const* outputs,
    uint32_t num_output_channels,
    uint32_t frames
) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }

    if (!inputs || !outputs || frames == 0) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    if (frames > plugin->max_block_size) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // Note: Channel count and pointer validation moved to Rust layer (public API)
    // C++ trusts that Rust has validated inputs correctly

    // Zero-copy: point input buffer list directly at caller's buffers
    const uint32_t byte_size = frames * sizeof(float);
    for (uint32_t ch = 0; ch < num_input_channels; ch++) {
        plugin->input_buffer_list->mBuffers[ch].mData = const_cast<float*>(inputs[ch]);
        plugin->input_buffer_list->mBuffers[ch].mDataByteSize = byte_size;
    }

    // Zero-copy: point output buffer list directly at caller's buffers
    for (uint32_t ch = 0; ch < num_output_channels; ch++) {
        plugin->output_buffer_list->mBuffers[ch].mData = outputs[ch];
        plugin->output_buffer_list->mBuffers[ch].mDataByteSize = byte_size;
    }

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

    if (status != noErr) {
        return RACK_AU_ERROR_AUDIO_UNIT + status;
    }

    // Zero-copy: AudioUnit already wrote directly to caller's output buffers

    // Update sample position for next call
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
// Channel Count Query
// ============================================================================

int rack_au_plugin_get_input_channels(RackAUPlugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return 0;
    }
    return static_cast<int>(plugin->input_channels);
}

int rack_au_plugin_get_output_channels(RackAUPlugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return 0;
    }
    return static_cast<int>(plugin->output_channels);
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
