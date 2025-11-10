#include "rack_vst3.h"
#include "public.sdk/source/vst/hosting/module.h"
#include "public.sdk/source/vst/hosting/plugprovider.h"
#include "public.sdk/source/vst/hosting/hostclasses.h"
#include "public.sdk/source/vst/hosting/processdata.h"
#include "public.sdk/source/vst/hosting/parameterchanges.h"
#include "public.sdk/source/vst/hosting/eventlist.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivstcomponent.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/ivstprocesscontext.h"
#include "pluginterfaces/vst/ivstunits.h"

#include <vector>
#include <string>
#include <cstring>
#include <mutex>

using namespace VST3;
using namespace Steinberg;
using namespace Steinberg::Vst;

// Global mutex for VST3 lifecycle operations
// VST3 module loading/unloading is not guaranteed to be thread-safe
static std::mutex g_vst3_lifecycle_mutex;

// Helper: Convert UTF-16 to UTF-8
// VST3 uses char16 (UTF-16) for strings
static std::string utf16_to_utf8(const char16* utf16_str) {
    if (!utf16_str) {
        return "";
    }

    std::string result;
    result.reserve(128); // Reserve reasonable space

    while (*utf16_str) {
        char16 c = *utf16_str++;

        // Handle UTF-16 to UTF-8 conversion
        if (c < 0x80) {
            // ASCII - 1 byte
            result.push_back(static_cast<char>(c));
        } else if (c < 0x800) {
            // 2 bytes
            result.push_back(static_cast<char>(0xC0 | (c >> 6)));
            result.push_back(static_cast<char>(0x80 | (c & 0x3F)));
        } else if (c >= 0xD800 && c <= 0xDBFF) {
            // High surrogate - need to read low surrogate
            if (*utf16_str >= 0xDC00 && *utf16_str <= 0xDFFF) {
                // Valid surrogate pair
                uint32_t codepoint = 0x10000 + ((c & 0x3FF) << 10) + (*utf16_str++ & 0x3FF);
                // 4 bytes
                result.push_back(static_cast<char>(0xF0 | (codepoint >> 18)));
                result.push_back(static_cast<char>(0x80 | ((codepoint >> 12) & 0x3F)));
                result.push_back(static_cast<char>(0x80 | ((codepoint >> 6) & 0x3F)));
                result.push_back(static_cast<char>(0x80 | (codepoint & 0x3F)));
            } else {
                // Invalid surrogate pair - replace with replacement character
                result.push_back('?');
            }
        } else if (c >= 0xDC00 && c <= 0xDFFF) {
            // Low surrogate without high surrogate - invalid
            result.push_back('?');
        } else {
            // 3 bytes
            result.push_back(static_cast<char>(0xE0 | (c >> 12)));
            result.push_back(static_cast<char>(0x80 | ((c >> 6) & 0x3F)));
            result.push_back(static_cast<char>(0x80 | (c & 0x3F)));
        }
    }

    return result;
}

// Helper: Convert hex string to UID
static bool string_to_uid(const char* str, VST3::UID& uid) {
    if (!str) {
        return false;
    }
    auto uid_opt = VST3::UID::fromString(std::string(str));
    if (!uid_opt) {
        return false;
    }
    uid = *uid_opt;
    return true;
}

// Internal plugin state
struct RackVST3Plugin {
    // Module and factory
    Hosting::Module::Ptr module;

    // Component and controller
    IPtr<IComponent> component;
    IPtr<IAudioProcessor> processor;
    IPtr<IEditController> controller;

    // Connection proxy (if component != controller)
    IPtr<IConnectionPoint> component_cp;
    IPtr<IConnectionPoint> controller_cp;

    // Plugin info
    std::string path;
    VST3::UID uid;

    // Audio configuration
    double sample_rate = 0.0;
    uint32_t max_block_size = 0;
    bool initialized = false;

    // I/O configuration
    int32 num_input_channels = 0;
    int32 num_output_channels = 0;

    // Processing structures
    HostProcessData process_data;
    ParameterChanges input_param_changes;
    ParameterChanges output_param_changes;
    EventList input_events;
    EventList output_events;

    // Audio buffers (for pointer arrays)
    std::vector<float*> input_ptrs;
    std::vector<float*> output_ptrs;

    // Parameter cache
    struct ParameterInfo {
        ParamID id;
        std::string title;
        std::string units;
        ParamValue min_value;
        ParamValue max_value;
        ParamValue default_value;
    };
    std::vector<ParameterInfo> parameters;
};

// ============================================================================
// Plugin Instance Implementation
// ============================================================================

RackVST3Plugin* rack_vst3_plugin_new(const char* path, const char* uid) {
    if (!path || !uid) {
        return nullptr;
    }

    std::lock_guard<std::mutex> lock(g_vst3_lifecycle_mutex);

    auto plugin = new(std::nothrow) RackVST3Plugin();
    if (!plugin) {
        return nullptr;
    }

    plugin->path = path;

    // Parse UID
    if (!string_to_uid(uid, plugin->uid)) {
        delete plugin;
        return nullptr;
    }

    // Load module
    std::string error_description;
    plugin->module = Hosting::Module::create(path, error_description);
    if (!plugin->module) {
        delete plugin;
        return nullptr;
    }

    // Create component
    const auto& factory = plugin->module->getFactory();
    plugin->component = factory.createInstance<IComponent>(plugin->uid);
    if (!plugin->component) {
        delete plugin;
        return nullptr;
    }

    // Get processor interface
    plugin->processor = U::cast<IAudioProcessor>(plugin->component);
    if (!plugin->processor) {
        delete plugin;
        return nullptr;
    }

    // Initialize component
    if (plugin->component->initialize(FUnknownPtr<IHostApplication>(new HostApplication())) != kResultOk) {
        delete plugin;
        return nullptr;
    }

    // Try to get edit controller
    TUID controllerCID;
    if (plugin->component->getControllerClassId(controllerCID) == kResultTrue) {
        // Controller is separate from component
        VST3::UID controllerUID = VST3::UID::fromTUID(controllerCID);

        plugin->controller = factory.createInstance<IEditController>(controllerUID);
        if (plugin->controller) {
            plugin->controller->initialize(FUnknownPtr<IHostApplication>(new HostApplication()));
        }
    } else {
        // Component is also the controller (single component architecture)
        plugin->controller = U::cast<IEditController>(plugin->component);
    }

    // Set up connection points if controller is separate
    if (plugin->controller && reinterpret_cast<void*>(plugin->controller.get()) != reinterpret_cast<void*>(plugin->component.get())) {
        plugin->component_cp = U::cast<IConnectionPoint>(plugin->component);
        plugin->controller_cp = U::cast<IConnectionPoint>(plugin->controller);

        if (plugin->component_cp && plugin->controller_cp) {
            plugin->component_cp->connect(plugin->controller_cp);
            plugin->controller_cp->connect(plugin->component_cp);
        }
    }

    return plugin;
}

void rack_vst3_plugin_free(RackVST3Plugin* plugin) {
    if (!plugin) {
        return;
    }

    std::lock_guard<std::mutex> lock(g_vst3_lifecycle_mutex);

    // Deactivate if active
    if (plugin->initialized && plugin->component) {
        plugin->component->setActive(false);
    }

    // Disconnect connection points
    if (plugin->component_cp && plugin->controller_cp) {
        plugin->component_cp->disconnect(plugin->controller_cp);
        plugin->controller_cp->disconnect(plugin->component_cp);
    }

    // Terminate controller
    if (plugin->controller && reinterpret_cast<void*>(plugin->controller.get()) != reinterpret_cast<void*>(plugin->component.get())) {
        plugin->controller->terminate();
        plugin->controller = nullptr;
    }

    // Terminate component
    if (plugin->component) {
        plugin->component->terminate();
        plugin->component = nullptr;
    }

    plugin->processor = nullptr;
    plugin->module = nullptr;

    delete plugin;
}

int rack_vst3_plugin_initialize(RackVST3Plugin* plugin, double sample_rate, uint32_t max_block_size) {
    if (!plugin || !plugin->component || !plugin->processor) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    std::lock_guard<std::mutex> lock(g_vst3_lifecycle_mutex);

    plugin->sample_rate = sample_rate;
    plugin->max_block_size = max_block_size;

    // Setup processing (stereo for now - TODO: make configurable)
    ProcessSetup setup;
    setup.processMode = kRealtime;
    setup.symbolicSampleSize = kSample32;
    setup.maxSamplesPerBlock = max_block_size;
    setup.sampleRate = sample_rate;

    if (plugin->processor->setupProcessing(setup) != kResultOk) {
        return RACK_VST3_ERROR_GENERIC;
    }

    // Get bus configuration
    int32 numInputBuses = plugin->component->getBusCount(kAudio, kInput);
    int32 numOutputBuses = plugin->component->getBusCount(kAudio, kOutput);

    // Activate main audio buses
    if (numInputBuses > 0) {
        plugin->component->activateBus(kAudio, kInput, 0, true);
        BusInfo busInfo;
        if (plugin->component->getBusInfo(kAudio, kInput, 0, busInfo) == kResultOk) {
            plugin->num_input_channels = busInfo.channelCount;
        }
    }

    if (numOutputBuses > 0) {
        plugin->component->activateBus(kAudio, kOutput, 0, true);
        BusInfo busInfo;
        if (plugin->component->getBusInfo(kAudio, kOutput, 0, busInfo) == kResultOk) {
            plugin->num_output_channels = busInfo.channelCount;
        }
    }

    // Activate component
    if (plugin->component->setActive(true) != kResultOk) {
        return RACK_VST3_ERROR_GENERIC;
    }

    // Start processing
    if (plugin->processor->setProcessing(true) != kResultOk) {
        plugin->component->setActive(false);
        return RACK_VST3_ERROR_GENERIC;
    }

    // Prepare process_data once during initialization (not in hot path)
    plugin->process_data.prepare(*plugin->component, max_block_size, kSample32);

    // Build parameter cache
    if (plugin->controller) {
        int32 param_count = plugin->controller->getParameterCount();
        plugin->parameters.clear();
        plugin->parameters.reserve(param_count);

        for (int32 i = 0; i < param_count; ++i) {
            ParameterInfo vst3_param_info;
            if (plugin->controller->getParameterInfo(i, vst3_param_info) == kResultOk) {
                RackVST3Plugin::ParameterInfo info;
                info.id = vst3_param_info.id;

                // Convert UTF-16 to UTF-8 (proper conversion for international characters)
                info.title = utf16_to_utf8(vst3_param_info.title);
                info.units = utf16_to_utf8(vst3_param_info.units);

                // VST3 parameters are already normalized 0.0-1.0
                info.min_value = 0.0;
                info.max_value = 1.0;
                info.default_value = vst3_param_info.defaultNormalizedValue;

                plugin->parameters.push_back(info);
            }
        }
    }

    plugin->initialized = true;
    return RACK_VST3_OK;
}

int rack_vst3_plugin_is_initialized(RackVST3Plugin* plugin) {
    return (plugin && plugin->initialized) ? 1 : 0;
}

int rack_vst3_plugin_reset(RackVST3Plugin* plugin) {
    if (!plugin || !plugin->initialized || !plugin->component) {
        return RACK_VST3_ERROR_NOT_INITIALIZED;
    }

    // VST3 doesn't have a direct "reset" like AudioUnit
    // We can deactivate and reactivate the component
    std::lock_guard<std::mutex> lock(g_vst3_lifecycle_mutex);

    plugin->component->setActive(false);
    if (plugin->component->setActive(true) != kResultOk) {
        return RACK_VST3_ERROR_GENERIC;
    }

    return RACK_VST3_OK;
}

int rack_vst3_plugin_get_input_channels(RackVST3Plugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return 0;
    }
    return plugin->num_input_channels;
}

int rack_vst3_plugin_get_output_channels(RackVST3Plugin* plugin) {
    if (!plugin || !plugin->initialized) {
        return 0;
    }
    return plugin->num_output_channels;
}

int rack_vst3_plugin_process(
    RackVST3Plugin* plugin,
    const float* const* inputs,
    uint32_t num_input_channels,
    float* const* outputs,
    uint32_t num_output_channels,
    uint32_t frames)
{
    if (!plugin || !plugin->initialized || !plugin->processor) {
        return RACK_VST3_ERROR_NOT_INITIALIZED;
    }

    // Update dynamic fields only (prepare() was called during initialization)
    plugin->process_data.numSamples = frames;

    // Set input buffers
    if (num_input_channels > 0 && inputs) {
        AudioBusBuffers& bus = plugin->process_data.inputs[0];
        bus.numChannels = num_input_channels;
        bus.channelBuffers32 = const_cast<float**>(inputs);
    }

    // Set output buffers
    if (num_output_channels > 0 && outputs) {
        AudioBusBuffers& bus = plugin->process_data.outputs[0];
        bus.numChannels = num_output_channels;
        bus.channelBuffers32 = const_cast<float**>(outputs);
    }

    // Set parameter and event interfaces
    plugin->process_data.inputParameterChanges = &plugin->input_param_changes;
    plugin->process_data.outputParameterChanges = &plugin->output_param_changes;
    plugin->process_data.inputEvents = &plugin->input_events;
    plugin->process_data.outputEvents = &plugin->output_events;

    // Process
    tresult result = plugin->processor->process(plugin->process_data);

    // Clear input/output events and parameter changes for next call
    plugin->input_events.clear();
    plugin->input_param_changes.clearQueue();
    plugin->output_events.clear();
    plugin->output_param_changes.clearQueue();

    return (result == kResultOk) ? RACK_VST3_OK : RACK_VST3_ERROR_GENERIC;
}

// ============================================================================
// Parameter API
// ============================================================================

int rack_vst3_plugin_parameter_count(RackVST3Plugin* plugin) {
    if (!plugin || !plugin->controller) {
        return 0;
    }
    return static_cast<int>(plugin->parameters.size());
}

int rack_vst3_plugin_get_parameter(RackVST3Plugin* plugin, uint32_t index, float* value) {
    if (!plugin || !plugin->controller || !value) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    if (index >= plugin->parameters.size()) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    ParamID param_id = plugin->parameters[index].id;
    ParamValue normalized = plugin->controller->getParamNormalized(param_id);
    *value = static_cast<float>(normalized);

    return RACK_VST3_OK;
}

int rack_vst3_plugin_set_parameter(RackVST3Plugin* plugin, uint32_t index, float value) {
    if (!plugin || !plugin->controller) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    if (index >= plugin->parameters.size()) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    ParamID param_id = plugin->parameters[index].id;
    plugin->controller->setParamNormalized(param_id, value);

    // Notify component of parameter change
    if (plugin->component_cp) {
        // In a full implementation, we'd send a message through the connection point
        // For now, the parameter change will be picked up in the next process() call
    }

    return RACK_VST3_OK;
}

int rack_vst3_plugin_parameter_info(
    RackVST3Plugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    float* min,
    float* max,
    float* default_value,
    char* unit,
    size_t unit_size)
{
    if (!plugin || !plugin->controller || !name || name_size == 0) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    if (index >= plugin->parameters.size()) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    const auto& param_info = plugin->parameters[index];

    // Name
    strncpy(name, param_info.title.c_str(), name_size - 1);
    name[name_size - 1] = '\0';

    // Min/max/default
    if (min) *min = static_cast<float>(param_info.min_value);
    if (max) *max = static_cast<float>(param_info.max_value);
    if (default_value) *default_value = static_cast<float>(param_info.default_value);

    // Unit
    if (unit && unit_size > 0) {
        strncpy(unit, param_info.units.c_str(), unit_size - 1);
        unit[unit_size - 1] = '\0';
    }

    return RACK_VST3_OK;
}

// ============================================================================
// Preset Management (Stub - TODO: Implement)
// ============================================================================

int rack_vst3_plugin_get_preset_count(RackVST3Plugin* plugin) {
    // VST3 presets are handled through IUnitInfo interface
    // TODO: Implement preset enumeration
    return 0;
}

int rack_vst3_plugin_get_preset_info(
    RackVST3Plugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    int32_t* preset_number)
{
    // TODO: Implement
    return RACK_VST3_ERROR_NOT_FOUND;
}

int rack_vst3_plugin_load_preset(RackVST3Plugin* plugin, int32_t preset_number) {
    // TODO: Implement
    return RACK_VST3_ERROR_NOT_FOUND;
}

int rack_vst3_plugin_get_state_size(RackVST3Plugin* plugin) {
    // VST3 state is handled through IBStream
    // Size is not known in advance
    // Return a reasonable maximum for now
    return 1024 * 1024;  // 1 MB max
}

int rack_vst3_plugin_get_state(RackVST3Plugin* plugin, uint8_t* data, size_t* size) {
    // TODO: Implement using IComponent::getState()
    if (!plugin || !data || !size) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }
    return RACK_VST3_ERROR_GENERIC;
}

int rack_vst3_plugin_set_state(RackVST3Plugin* plugin, const uint8_t* data, size_t size) {
    // TODO: Implement using IComponent::setState()
    if (!plugin || !data) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }
    return RACK_VST3_ERROR_GENERIC;
}

// ============================================================================
// MIDI API
// ============================================================================

int rack_vst3_plugin_send_midi(
    RackVST3Plugin* plugin,
    const RackVST3MidiEvent* events,
    uint32_t event_count)
{
    if (!plugin || !events) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    // Convert MIDI events to VST3 events
    for (uint32_t i = 0; i < event_count; ++i) {
        const auto& midi_event = events[i];

        Event vst3_event;
        memset(&vst3_event, 0, sizeof(Event));
        vst3_event.sampleOffset = midi_event.sample_offset;
        vst3_event.busIndex = 0;

        uint8_t status = midi_event.status & 0xF0;

        switch (status) {
            case 0x90:  // Note On
                vst3_event.type = Event::kNoteOnEvent;
                vst3_event.noteOn.channel = midi_event.channel;
                vst3_event.noteOn.pitch = midi_event.data1;
                vst3_event.noteOn.velocity = static_cast<float>(midi_event.data2) / 127.0f;
                vst3_event.noteOn.noteId = -1;  // Not specified
                break;

            case 0x80:  // Note Off
                vst3_event.type = Event::kNoteOffEvent;
                vst3_event.noteOff.channel = midi_event.channel;
                vst3_event.noteOff.pitch = midi_event.data1;
                vst3_event.noteOff.velocity = static_cast<float>(midi_event.data2) / 127.0f;
                vst3_event.noteOff.noteId = -1;  // Not specified
                break;

            default:
                // For other MIDI events, we'd need to use LegacyMIDICCOutEvent
                // or implement proper conversion
                // TODO: Implement full MIDI event conversion
                continue;
        }

        plugin->input_events.addEvent(vst3_event);
    }

    return RACK_VST3_OK;
}
