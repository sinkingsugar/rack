#include "rack_au.h"
#include <AudioToolbox/AudioToolbox.h>
#include <CoreFoundation/CoreFoundation.h>
#include <cstring>
#include <cstdio>  // for sscanf

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
    // on the calling thread before AudioUnitRender returns, so no race condition
    const float* current_input;
};

// ============================================================================
// Plugin Instance Implementation
// ============================================================================

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
    if (ioData->mNumberBuffers >= 2 &&
        ioData->mBuffers[0].mData &&
        ioData->mBuffers[1].mData) {

        float* left_out = static_cast<float*>(ioData->mBuffers[0].mData);
        float* right_out = static_cast<float*>(ioData->mBuffers[1].mData);
        const float* interleaved = plugin->current_input;

        for (UInt32 i = 0; i < inNumberFrames; i++) {
            left_out[i] = interleaved[i * 2];
            right_out[i] = interleaved[i * 2 + 1];
        }
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
    OSStatus status = AudioComponentInstanceNew(component, &plugin->audio_unit);
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
        AudioUnitUninitialize(plugin->audio_unit);
        AudioComponentInstanceDispose(plugin->audio_unit);
    }

    // Free audio buffers - free ALL channels, not just buffer[0]
    if (plugin->input_buffer_list) {
        for (UInt32 i = 0; i < plugin->input_buffer_list->mNumberBuffers; i++) {
            if (plugin->input_buffer_list->mBuffers[i].mData) {
                free(plugin->input_buffer_list->mBuffers[i].mData);
            }
        }
        free(plugin->input_buffer_list);
    }

    if (plugin->output_buffer_list) {
        for (UInt32 i = 0; i < plugin->output_buffer_list->mNumberBuffers; i++) {
            if (plugin->output_buffer_list->mBuffers[i].mData) {
                free(plugin->output_buffer_list->mBuffers[i].mData);
            }
        }
        free(plugin->output_buffer_list);
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
        plugin->input_buffer_list->mBuffers[i].mData = malloc(buffer_size);
        if (!plugin->input_buffer_list->mBuffers[i].mData) {
            // Clean up partially allocated buffers
            for (UInt32 j = 0; j < i; j++) {
                free(plugin->input_buffer_list->mBuffers[j].mData);
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
            free(plugin->input_buffer_list->mBuffers[i].mData);
        }
        free(plugin->input_buffer_list);
        plugin->input_buffer_list = nullptr;
        return RACK_AU_ERROR_GENERIC;  // Memory allocation failed
    }
    plugin->output_buffer_list->mNumberBuffers = 2;

    for (UInt32 i = 0; i < 2; i++) {
        plugin->output_buffer_list->mBuffers[i].mNumberChannels = 1;
        plugin->output_buffer_list->mBuffers[i].mDataByteSize = buffer_size;
        plugin->output_buffer_list->mBuffers[i].mData = malloc(buffer_size);
        if (!plugin->output_buffer_list->mBuffers[i].mData) {
            // Clean up all allocated buffers
            for (UInt32 j = 0; j < i; j++) {
                free(plugin->output_buffer_list->mBuffers[j].mData);
            }
            free(plugin->output_buffer_list);
            for (UInt32 j = 0; j < 2; j++) {
                free(plugin->input_buffer_list->mBuffers[j].mData);
            }
            free(plugin->input_buffer_list);
            plugin->input_buffer_list = nullptr;
            plugin->output_buffer_list = nullptr;
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
    status = AudioUnitInitialize(plugin->audio_unit);
    if (status != noErr) {
        // Clean up buffers on failure
        if (plugin->input_buffer_list) {
            for (UInt32 i = 0; i < 2; i++) {
                free(plugin->input_buffer_list->mBuffers[i].mData);
            }
            free(plugin->input_buffer_list);
            plugin->input_buffer_list = nullptr;
        }
        if (plugin->output_buffer_list) {
            for (UInt32 i = 0; i < 2; i++) {
                free(plugin->output_buffer_list->mBuffers[i].mData);
            }
            free(plugin->output_buffer_list);
            plugin->output_buffer_list = nullptr;
        }
        return RACK_AU_ERROR_AUDIO_UNIT + status;
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

    // Set up AudioTimeStamp
    AudioTimeStamp timestamp;
    memset(&timestamp, 0, sizeof(timestamp));
    timestamp.mFlags = kAudioTimeStampSampleTimeValid;
    timestamp.mSampleTime = 0;

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

        for (uint32_t i = 0; i < frames; i++) {
            output[i * 2] = left_in[i];
            output[i * 2 + 1] = right_in[i];
        }
    } else {
        // Buffer validation failed - return silence
        memset(output, 0, frames * 2 * sizeof(float));
    }

    return RACK_AU_OK;
}

int rack_au_plugin_parameter_count(RackAUPlugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return 0;
    }
    
    // TODO: Query actual parameter count from AudioUnit
    return 0;
}

int rack_au_plugin_get_parameter(RackAUPlugin* plugin, uint32_t index, float* value) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }
    
    if (!value) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }
    
    // TODO: Get actual parameter value from AudioUnit
    *value = 0.0f;
    return RACK_AU_ERROR_INVALID_PARAM;
}

int rack_au_plugin_set_parameter(RackAUPlugin* plugin, uint32_t index, float value) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }
    
    // TODO: Set actual parameter value on AudioUnit
    return RACK_AU_ERROR_INVALID_PARAM;
}

int rack_au_plugin_parameter_info(
    RackAUPlugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    float* min,
    float* max,
    float* default_value
) {
    if (!plugin || !plugin->initialized) {
        return RACK_AU_ERROR_NOT_INITIALIZED;
    }
    
    if (!name || !min || !max || !default_value) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }
    
    // TODO: Get actual parameter info from AudioUnit
    return RACK_AU_ERROR_INVALID_PARAM;
}
