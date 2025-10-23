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
};

// ============================================================================
// Plugin Instance Implementation
// ============================================================================

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

    // Initialize the AudioUnit
    status = AudioUnitInitialize(plugin->audio_unit);
    if (status != noErr) {
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
    
    // TODO: Implement actual audio processing
    // For now, just pass through
    memcpy(output, input, frames * 2 * sizeof(float));  // 2 channels (stereo)
    
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
