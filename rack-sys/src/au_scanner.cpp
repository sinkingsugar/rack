#include "rack_au.h"
#include <AudioToolbox/AudioToolbox.h>
#include <CoreFoundation/CoreFoundation.h>
#include <vector>
#include <string>
#include <cstring>

// Internal scanner state
struct RackAUScanner {
    std::vector<AudioComponent> components;
};

// Helper: Convert CFString to C string
static bool CFStringToCString(CFStringRef cfstr, char* buffer, size_t buffer_size) {
    if (!cfstr || !buffer || buffer_size == 0) {
        return false;
    }
    
    return CFStringGetCString(cfstr, buffer, buffer_size, kCFStringEncodingUTF8);
}

// Helper: Convert AudioUnit type to our enum
static RackAUPluginType AudioUnitTypeToPluginType(OSType type) {
    switch (type) {
        case kAudioUnitType_Effect:
        case kAudioUnitType_MusicEffect:
            return RACK_AU_TYPE_EFFECT;
        case kAudioUnitType_MusicDevice:
            return RACK_AU_TYPE_INSTRUMENT;
        case kAudioUnitType_Mixer:
            return RACK_AU_TYPE_MIXER;
        case kAudioUnitType_FormatConverter:
            return RACK_AU_TYPE_FORMAT_CONVERTER;
        default:
            return RACK_AU_TYPE_OTHER;
    }
}

// Helper: Create unique ID string from AudioComponentDescription
static void CreateUniqueID(const AudioComponentDescription& desc, char* buffer, size_t buffer_size) {
    snprintf(buffer, buffer_size, "%08X-%08X-%08X",
             (unsigned int)desc.componentType,
             (unsigned int)desc.componentSubType,
             (unsigned int)desc.componentManufacturer);
}

// ============================================================================
// Scanner Implementation
// ============================================================================

RackAUScanner* rack_au_scanner_new(void) {
    return new RackAUScanner();
}

void rack_au_scanner_free(RackAUScanner* scanner) {
    delete scanner;
}

int rack_au_scanner_scan(RackAUScanner* scanner, RackAUPluginInfo* plugins, size_t max_plugins) {
    if (!scanner) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    // If plugins is NULL, we're just counting
    bool count_only = (plugins == nullptr);

    scanner->components.clear();
    size_t count = 0;
    
    // Enumerate all AudioUnit components
    AudioComponentDescription desc = {0};
    desc.componentType = 0;  // 0 means "any type"
    desc.componentSubType = 0;
    desc.componentManufacturer = 0;
    
    AudioComponent comp = nullptr;
    while ((comp = AudioComponentFindNext(comp, &desc)) != nullptr) {
        // Get component description
        AudioComponentDescription foundDesc;
        OSStatus status = AudioComponentGetDescription(comp, &foundDesc);
        if (status != noErr) {
            continue;
        }

        // If we're just counting, don't need to get details
        if (count_only) {
            scanner->components.push_back(comp);
            count++;
            continue;
        }

        // If array is full, continue counting but don't fill
        if (count >= max_plugins) {
            count++;
            continue;
        }

        // Get component name
        CFStringRef name = nullptr;
        status = AudioComponentCopyName(comp, &name);
        if (status != noErr || !name) {
            continue;
        }

        // Fill in plugin info
        RackAUPluginInfo& info = plugins[count];
        
        // Name
        if (!CFStringToCString(name, info.name, sizeof(info.name))) {
            snprintf(info.name, sizeof(info.name), "<unknown>");
        }
        CFRelease(name);
        
        // Manufacturer (convert OSType to string)
        OSType mfg = foundDesc.componentManufacturer;
        if (mfg == kAudioUnitManufacturer_Apple) {
            snprintf(info.manufacturer, sizeof(info.manufacturer), "Apple");
        } else {
            // Convert FourCC to readable string with validation
            char mfgStr[5] = {0};
            unsigned char bytes[4] = {
                static_cast<unsigned char>((mfg >> 24) & 0xFF),
                static_cast<unsigned char>((mfg >> 16) & 0xFF),
                static_cast<unsigned char>((mfg >> 8) & 0xFF),
                static_cast<unsigned char>(mfg & 0xFF)
            };
            for (int i = 0; i < 4; ++i) {
                // Printable ASCII range: 0x20 (space) to 0x7E (~)
                mfgStr[i] = (bytes[i] >= 0x20 && bytes[i] <= 0x7E) ? bytes[i] : '?';
            }
            snprintf(info.manufacturer, sizeof(info.manufacturer), "%s", mfgStr);
        }

        // Path (AudioUnits are system-registered, path not easily accessible)
        // We use a placeholder - the unique_id is what matters for loading
        snprintf(info.path, sizeof(info.path), "<system>");

        // Unique ID
        CreateUniqueID(foundDesc, info.unique_id, sizeof(info.unique_id));

        // Version
        info.version = foundDesc.componentFlags;
        
        // Type
        info.plugin_type = AudioUnitTypeToPluginType(foundDesc.componentType);
        
        // Store component for later loading
        scanner->components.push_back(comp);
        
        count++;
    }
    
    return static_cast<int>(count);
}
