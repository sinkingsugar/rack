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
#include "pluginterfaces/base/ibstream.h"
#include "pluginterfaces/vst/ivsthostapplication.h"

#include <vector>
#include <string>
#include <cstring>
#include <mutex>
#include <algorithm>

using namespace VST3;
using namespace Steinberg;
using namespace Steinberg::Vst;

// Global mutex for VST3 lifecycle operations
// VST3 module loading/unloading is not guaranteed to be thread-safe
static std::mutex g_vst3_lifecycle_mutex;

// Helper: Convert UTF-16 to UTF-8
// VST3 uses char16 (UTF-16) for strings
// Handles surrogate pairs and malformed input safely
static std::string utf16_to_utf8(const char16* utf16_str) {
    if (!utf16_str) {
        return "";
    }

    std::string result;
    result.reserve(128);

    // Safety limit to prevent infinite loops on corrupted data
    // VST3 spec limits plugin names to 256 chars
    const size_t MAX_STRING_LENGTH = 4096;
    size_t processed = 0;

    while (*utf16_str && processed < MAX_STRING_LENGTH) {
        char16 c = *utf16_str++;
        processed++;

        // UTF-16 to UTF-8 conversion with complete error handling
        if (c < 0x80) {
            // U+0000 to U+007F: ASCII range (1 byte in UTF-8)
            result.push_back(static_cast<char>(c));
        }
        else if (c < 0x800) {
            // U+0080 to U+07FF: 2 bytes in UTF-8
            result.push_back(static_cast<char>(0xC0 | (c >> 6)));
            result.push_back(static_cast<char>(0x80 | (c & 0x3F)));
        }
        else if (c >= 0xD800 && c <= 0xDBFF) {
            // High surrogate (U+D800 to U+DBFF): Start of surrogate pair
            // Must be followed by low surrogate to form valid codepoint

            // Safety check: Ensure we haven't reached end of string
            if (*utf16_str == 0) {
                // Malformed: High surrogate at end of string
                result.append("\xEF\xBF\xBD"); // UTF-8 replacement character U+FFFD
                break;
            }

            char16 low = *utf16_str;

            // Validate low surrogate range (U+DC00 to U+DFFF)
            if (low >= 0xDC00 && low <= 0xDFFF) {
                // Valid surrogate pair - decode to codepoint U+10000 to U+10FFFF
                utf16_str++;
                processed++;

                uint32_t high_bits = (c & 0x3FF);      // 10 bits from high surrogate
                uint32_t low_bits = (low & 0x3FF);     // 10 bits from low surrogate
                uint32_t codepoint = 0x10000 + (high_bits << 10) + low_bits;

                // Encode as 4-byte UTF-8 sequence
                result.push_back(static_cast<char>(0xF0 | (codepoint >> 18)));
                result.push_back(static_cast<char>(0x80 | ((codepoint >> 12) & 0x3F)));
                result.push_back(static_cast<char>(0x80 | ((codepoint >> 6) & 0x3F)));
                result.push_back(static_cast<char>(0x80 | (codepoint & 0x3F)));
            } else {
                // Malformed: High surrogate not followed by low surrogate
                result.append("\xEF\xBF\xBD"); // UTF-8 replacement character U+FFFD
            }
        }
        else if (c >= 0xDC00 && c <= 0xDFFF) {
            // Malformed: Low surrogate (U+DC00 to U+DFFF) without preceding high surrogate
            result.append("\xEF\xBF\xBD"); // UTF-8 replacement character U+FFFD
        }
        else {
            // U+0800 to U+FFFF (excluding surrogates): 3 bytes in UTF-8
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

// Memory stream implementation for state serialization
class MemoryStream : public IBStream {
public:
    MemoryStream() : ref_count_(1), position_(0) {}
    MemoryStream(const uint8_t* data, size_t size) : ref_count_(1), position_(0) {
        buffer_.assign(data, data + size);
    }

    virtual ~MemoryStream() = default;

    // IUnknown
    DECLARE_FUNKNOWN_METHODS

    // IBStream
    tresult PLUGIN_API read(void* buffer, int32 numBytes, int32* numBytesRead) override {
        if (!buffer || numBytes < 0) {
            return kInvalidArgument;
        }

        int32 available = static_cast<int32>(buffer_.size()) - position_;
        int32 to_read = std::min(numBytes, available);

        if (to_read > 0) {
            memcpy(buffer, buffer_.data() + position_, to_read);
            position_ += to_read;
        }

        if (numBytesRead) {
            *numBytesRead = to_read;
        }

        return to_read == numBytes ? kResultOk : kResultFalse;
    }

    tresult PLUGIN_API write(void* buffer, int32 numBytes, int32* numBytesWritten) override {
        if (!buffer || numBytes < 0) {
            return kInvalidArgument;
        }

        // Resize buffer if needed
        if (position_ + numBytes > static_cast<int32>(buffer_.size())) {
            buffer_.resize(position_ + numBytes);
        }

        memcpy(buffer_.data() + position_, buffer, numBytes);
        position_ += numBytes;

        if (numBytesWritten) {
            *numBytesWritten = numBytes;
        }

        return kResultOk;
    }

    tresult PLUGIN_API seek(int64 pos, int32 mode, int64* result) override {
        switch (mode) {
            case kIBSeekSet:
                position_ = static_cast<int32>(pos);
                break;
            case kIBSeekCur:
                position_ += static_cast<int32>(pos);
                break;
            case kIBSeekEnd:
                position_ = static_cast<int32>(buffer_.size()) + static_cast<int32>(pos);
                break;
            default:
                return kInvalidArgument;
        }

        position_ = std::max(0, std::min(position_, static_cast<int32>(buffer_.size())));

        if (result) {
            *result = position_;
        }

        return kResultOk;
    }

    tresult PLUGIN_API tell(int64* pos) override {
        if (!pos) {
            return kInvalidArgument;
        }
        *pos = position_;
        return kResultOk;
    }

    // Accessors
    const std::vector<uint8_t>& getData() const { return buffer_; }
    size_t getSize() const { return buffer_.size(); }
    void clear() { buffer_.clear(); position_ = 0; }

private:
    std::atomic<uint32> ref_count_;
    std::vector<uint8_t> buffer_;
    int32 position_;
};

IMPLEMENT_REFCOUNT(MemoryStream)

tresult PLUGIN_API MemoryStream::queryInterface(const TUID _iid, void** obj) {
    QUERY_INTERFACE(_iid, obj, FUnknown::iid, IBStream)
    QUERY_INTERFACE(_iid, obj, IBStream::iid, IBStream)
    *obj = nullptr;
    return kNoInterface;
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

    // Preset cache (factory presets from IUnitInfo)
    struct PresetInfo {
        int32 program_list_id;
        int32 program_index;
        std::string name;
    };
    std::vector<PresetInfo> presets;
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

    // Setup processing with 32-bit float samples in realtime mode
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

    // Enumerate factory presets if available
    IPtr<IUnitInfo> unit_info = U::cast<IUnitInfo>(plugin->controller);
    if (unit_info) {
        int32 program_list_count = unit_info->getProgramListCount();
        for (int32 i = 0; i < program_list_count; ++i) {
            ProgramListInfo list_info;
            if (unit_info->getProgramListInfo(i, list_info) == kResultOk) {
                // Enumerate programs in this list
                for (int32 j = 0; j < list_info.programCount; ++j) {
                    String128 program_name;
                    if (unit_info->getProgramName(list_info.id, j, program_name) == kResultOk) {
                        RackVST3Plugin::PresetInfo preset;
                        preset.program_list_id = list_info.id;
                        preset.program_index = j;
                        preset.name = utf16_to_utf8(program_name);
                        plugin->presets.push_back(preset);
                    }
                }
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
    // Acquire mutex BEFORE checking state to prevent TOCTOU race condition
    // Without this, another thread could change initialized between check and lock
    std::lock_guard<std::mutex> lock(g_vst3_lifecycle_mutex);

    if (!plugin || !plugin->initialized || !plugin->component) {
        return RACK_VST3_ERROR_NOT_INITIALIZED;
    }

    // VST3 doesn't have a direct "reset" like AudioUnit
    // We can deactivate and reactivate the component
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

    // Validate input parameters to prevent buffer overruns
    // Channel counts must match what was configured during initialization
    if (num_input_channels != static_cast<uint32_t>(plugin->num_input_channels)) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }
    if (num_output_channels != static_cast<uint32_t>(plugin->num_output_channels)) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }
    // Frame count must not exceed the max block size configured during initialization
    if (frames > plugin->max_block_size) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    // Validate buffer pointers when channel counts > 0
    if (num_input_channels > 0 && !inputs) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }
    if (num_output_channels > 0 && !outputs) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    // Update dynamic fields only (prepare() was called during initialization)
    plugin->process_data.numSamples = frames;

    // Set input buffers
    if (num_input_channels > 0) {
        AudioBusBuffers& bus = plugin->process_data.inputs[0];
        bus.numChannels = num_input_channels;
        bus.channelBuffers32 = const_cast<float**>(inputs);
    }

    // Set output buffers
    if (num_output_channels > 0) {
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

    // Set parameter value on controller (for UI reflection)
    plugin->controller->setParamNormalized(param_id, value);

    // Queue parameter change for component notification in next process() call
    // This ensures the component receives the parameter change properly
    int32 queue_index = 0;
    IParamValueQueue* queue = plugin->input_param_changes.addParameterData(param_id, queue_index);
    if (queue) {
        // Add parameter change at sample offset 0 (beginning of next buffer)
        int32 point_index = 0;
        queue->addPoint(0, value, point_index);
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
    if (!plugin || !plugin->initialized) {
        return 0;
    }
    return static_cast<int>(plugin->presets.size());
}

int rack_vst3_plugin_get_preset_info(
    RackVST3Plugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    int32_t* preset_number)
{
    if (!plugin || !plugin->initialized || !name || name_size == 0) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    if (index >= plugin->presets.size()) {
        return RACK_VST3_ERROR_NOT_FOUND;
    }

    const auto& preset = plugin->presets[index];

    // Copy name
    strncpy(name, preset.name.c_str(), name_size - 1);
    name[name_size - 1] = '\0';

    // Preset number is just the index
    if (preset_number) {
        *preset_number = static_cast<int32_t>(index);
    }

    return RACK_VST3_OK;
}

int rack_vst3_plugin_load_preset(RackVST3Plugin* plugin, int32_t preset_number) {
    if (!plugin || !plugin->initialized || !plugin->controller) {
        return RACK_VST3_ERROR_NOT_INITIALIZED;
    }

    if (preset_number < 0 || preset_number >= static_cast<int32_t>(plugin->presets.size())) {
        return RACK_VST3_ERROR_NOT_FOUND;
    }

    const auto& preset = plugin->presets[preset_number];

    // VST3 preset loading is complex - requires IProgramListData interface
    // Get IProgramListData from IUnitInfo
    IPtr<IUnitInfo> unit_info = U::cast<IUnitInfo>(plugin->controller);
    if (!unit_info) {
        return RACK_VST3_ERROR_GENERIC;
    }

    // Try to get preset data using IProgramListData
    IPtr<IProgramListData> program_data = U::cast<IProgramListData>(unit_info);
    if (program_data) {
        // Create stream to receive preset data
        // IPtr<MemoryStream>(new MemoryStream(), false) explanation:
        //   - new MemoryStream() creates object with refCount=1 (COM convention)
        //   - IPtr(..., false) takes ownership WITHOUT calling addRef() (false = don't addRef)
        //   - IPtr destructor calls release() when going out of scope
        //   - This ensures cleanup on ALL code paths (success, error, early return)
        IPtr<MemoryStream> stream(new MemoryStream(), false);

        // Get program data
        tresult result = program_data->getProgramData(preset.program_list_id, preset.program_index, stream);
        if (result == kResultOk) {
            // Reset stream and apply the preset data
            stream->seek(0, IBStream::kIBSeekSet, nullptr);
            result = program_data->setProgramData(preset.program_list_id, preset.program_index, stream);

            if (result == kResultOk) {
                return RACK_VST3_OK;
            }
        }
        // IPtr automatically releases stream on scope exit
    }

    // Fallback 1: Try to find and set a "program" parameter
    // Some plugins expose preset selection as a regular parameter
    // Only use discrete/list parameters to avoid false positives
    int32 param_count = plugin->controller->getParameterCount();
    for (int32 i = 0; i < param_count; i++) {
        Vst::ParameterInfo param_info;
        if (plugin->controller->getParameterInfo(i, param_info) != kResultOk) {
            continue;
        }

        // Only consider discrete parameters (stepCount > 0) or parameters with kIsProgramChange flag
        bool is_discrete = param_info.stepCount > 0;
        bool is_program_change = (param_info.flags & ParameterInfo::kIsProgramChange) != 0;

        if (!is_discrete && !is_program_change) {
            continue; // Skip continuous parameters without program change flag
        }

        // Convert parameter title to UTF-8 and lowercase for comparison
        std::string param_title = utf16_to_utf8(param_info.title);
        std::transform(param_title.begin(), param_title.end(), param_title.begin(),
                      [](unsigned char c) { return std::tolower(c); });

        // More specific matching to reduce false positives:
        // - Must be discrete or marked as program change
        // - Title must contain complete words "program", "preset", or "patch"
        // - Check for word boundaries to avoid matching "programming", "unpresettable", etc.
        bool is_program_param = false;
        if (is_program_change) {
            // Explicitly marked as program change - always use
            is_program_param = true;
        } else {
            // Check for specific keywords as complete words
            size_t pos = 0;
            while ((pos = param_title.find_first_of("programpatchpreset", pos)) != std::string::npos) {
                // Check if it's a word boundary
                bool start_ok = (pos == 0 || !std::isalnum(param_title[pos - 1]));
                size_t len = 0;
                if (param_title.substr(pos, 7) == "program") len = 7;
                else if (param_title.substr(pos, 5) == "patch") len = 5;
                else if (param_title.substr(pos, 6) == "preset") len = 6;
                else { pos++; continue; }

                bool end_ok = (pos + len >= param_title.length() || !std::isalnum(param_title[pos + len]));
                if (start_ok && end_ok) {
                    is_program_param = true;
                    break;
                }
                pos += len;
            }
        }

        if (is_program_param) {
            // Verify program_index is within valid range
            if (preset.program_index > param_info.stepCount) {
                continue; // Index out of range for this parameter
            }

            // Map program_index to normalized value
            float normalized_value = static_cast<float>(preset.program_index) /
                                    static_cast<float>(param_info.stepCount);

            // Clamp to 0.0-1.0 range
            normalized_value = std::max(0.0f, std::min(1.0f, normalized_value));

            // Set the parameter
            ParamID param_id = param_info.id;
            if (plugin->controller->setParamNormalized(param_id, normalized_value) == kResultOk) {
                // Queue the parameter change for the processor
                int32 queue_index = 0;
                IParamValueQueue* queue = plugin->input_param_changes.addParameterData(param_id, queue_index);
                if (queue) {
                    int32 point_index = 0;
                    queue->addPoint(0, normalized_value, point_index);
                }

                // Successfully loaded via parameter-based fallback
                return RACK_VST3_OK;
            }
        }
    }

    // Fallback 2: Try using IUnitInfo to select the unit containing this program
    // Some plugins may respond to unit selection by loading the first program in that unit
    int32 unit_count = unit_info->getUnitCount();
    for (int32 i = 0; i < unit_count; i++) {
        UnitInfo unit_info_struct;
        if (unit_info->getUnitInfo(i, unit_info_struct) == kResultOk) {
            // Check if this unit's program list matches our preset
            if (unit_info_struct.programListId == preset.program_list_id) {
                // Try to select this unit (might trigger program load)
                tresult result = unit_info->selectUnit(unit_info_struct.id);
                if (result == kResultOk) {
                    // Unit selected - this might have loaded a program
                    // We can't verify, so return success as best effort
                    return RACK_VST3_OK;
                }
            }
        }
    }

    // No fallback succeeded - plugin doesn't support programmatic preset loading
    // This is not an error - it's a limitation of the plugin itself
    return RACK_VST3_ERROR_GENERIC;
}

int rack_vst3_plugin_get_state_size(RackVST3Plugin* plugin) {
    // VST3 state size is not known in advance
    // Return a reasonable maximum
    return 1024 * 1024;  // 1 MB max
}

int rack_vst3_plugin_get_state(RackVST3Plugin* plugin, uint8_t* data, size_t* size) {
    if (!plugin || !data || !size || !plugin->component) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    if (*size == 0) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    // Create memory stream for state serialization
    // IPtr ensures automatic cleanup on ALL code paths (success, error, buffer size check failure)
    IPtr<MemoryStream> stream(new MemoryStream(), false);

    // Get component state
    tresult result = plugin->component->getState(stream);
    if (result != kResultOk) {
        return RACK_VST3_ERROR_GENERIC;
    }

    // Record component state size for proper deserialization later
    int64 component_state_size = 0;
    stream->tell(&component_state_size);

    // Write component state size as a marker (uint32_t)
    uint32_t size_marker = static_cast<uint32_t>(component_state_size);
    stream->write(&size_marker, sizeof(size_marker), nullptr);

    // Get controller state if separate controller
    if (plugin->controller && reinterpret_cast<void*>(plugin->controller.get()) != reinterpret_cast<void*>(plugin->component.get())) {
        result = plugin->controller->getState(stream);
        if (result != kResultOk) {
            return RACK_VST3_ERROR_GENERIC;
        }
    }

    // Copy to output buffer
    // Note: MemoryStream grows dynamically - no fixed limit, can't overflow
    // The 1MB from get_state_size() is just a hint for buffer allocation
    // Here we check if the ACTUAL state fits in the caller's buffer
    size_t state_size = stream->getSize();
    if (state_size > *size) {
        *size = state_size;  // Return required size for caller to retry
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    memcpy(data, stream->getData().data(), state_size);
    *size = state_size;
    // IPtr automatically releases stream on scope exit

    return RACK_VST3_OK;
}

int rack_vst3_plugin_set_state(RackVST3Plugin* plugin, const uint8_t* data, size_t size) {
    if (!plugin || !data || size == 0 || !plugin->component) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    // Create memory stream from data (IPtr provides RAII cleanup)
    IPtr<MemoryStream> stream(new MemoryStream(data, size), false);

    // Set component state (reads from position 0)
    tresult result = plugin->component->setState(stream);
    if (result != kResultOk) {
        return RACK_VST3_ERROR_GENERIC;
    }

    // Set controller state if separate controller
    if (plugin->controller && reinterpret_cast<void*>(plugin->controller.get()) != reinterpret_cast<void*>(plugin->component.get())) {
        // Read the size marker to find where controller state begins
        uint32_t component_state_size = 0;
        int32 bytes_read = 0;
        result = stream->read(&component_state_size, sizeof(component_state_size), &bytes_read);

        if (result == kResultOk && bytes_read == sizeof(component_state_size)) {
            // Controller state starts right after the size marker
            // We're already at the correct position after reading the marker
            result = plugin->controller->setState(stream);
            if (result != kResultOk) {
                return RACK_VST3_ERROR_GENERIC;
            }
        } else {
            // No size marker found - might be old format or component-only state
            // This is OK, just skip controller restoration
        }
    }

    // IPtr automatically releases stream on scope exit
    return RACK_VST3_OK;
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
                plugin->input_events.addEvent(vst3_event);
                break;

            case 0x80:  // Note Off
                vst3_event.type = Event::kNoteOffEvent;
                vst3_event.noteOff.channel = midi_event.channel;
                vst3_event.noteOff.pitch = midi_event.data1;
                vst3_event.noteOff.velocity = static_cast<float>(midi_event.data2) / 127.0f;
                vst3_event.noteOff.noteId = -1;  // Not specified
                plugin->input_events.addEvent(vst3_event);
                break;

            case 0xA0:  // Polyphonic Key Pressure (Aftertouch)
                vst3_event.type = Event::kPolyPressureEvent;
                vst3_event.polyPressure.channel = midi_event.channel;
                vst3_event.polyPressure.pitch = midi_event.data1;
                vst3_event.polyPressure.pressure = static_cast<float>(midi_event.data2) / 127.0f;
                plugin->input_events.addEvent(vst3_event);
                break;

            case 0xB0:  // Control Change
                // Use LegacyMIDICCOutEvent for CC messages
                // This is the standard VST3 approach for MIDI CC data
                vst3_event.type = Event::kLegacyMIDICCOutEvent;
                vst3_event.midiCCOut.channel = midi_event.channel;
                vst3_event.midiCCOut.controlNumber = midi_event.data1;
                vst3_event.midiCCOut.value = midi_event.data2;
                vst3_event.midiCCOut.value2 = 0;
                plugin->input_events.addEvent(vst3_event);
                break;

            case 0xC0:  // Program Change
                // VST3 doesn't have a dedicated program change event type
                // Use LegacyMIDICCOutEvent with controlNumber >= 0x80 for non-CC MIDI
                // Note: Not all plugins may support this encoding
                vst3_event.type = Event::kLegacyMIDICCOutEvent;
                vst3_event.midiCCOut.channel = midi_event.channel;
                vst3_event.midiCCOut.controlNumber = 0x80;  // >= 0x80 indicates non-CC MIDI
                vst3_event.midiCCOut.value = midi_event.data1;
                vst3_event.midiCCOut.value2 = 0;
                plugin->input_events.addEvent(vst3_event);
                break;

            case 0xD0:  // Channel Pressure (Aftertouch)
                // VST3 doesn't have a dedicated channel pressure event type
                // Use LegacyMIDICCOutEvent with controlNumber >= 0x80 for non-CC MIDI
                // Note: Not all plugins may support this encoding
                vst3_event.type = Event::kLegacyMIDICCOutEvent;
                vst3_event.midiCCOut.channel = midi_event.channel;
                vst3_event.midiCCOut.controlNumber = 0x81;  // >= 0x80 indicates non-CC MIDI
                vst3_event.midiCCOut.value = midi_event.data1;
                vst3_event.midiCCOut.value2 = 0;
                plugin->input_events.addEvent(vst3_event);
                break;

            case 0xE0: {  // Pitch Bend
                // VST3 doesn't have a dedicated pitch bend event type
                // Use LegacyMIDICCOutEvent with controlNumber >= 0x80 for non-CC MIDI
                // Note: Not all plugins may support this encoding
                // The plugin should interpret value (LSB) and value2 (MSB) as 14-bit pitch bend
                vst3_event.type = Event::kLegacyMIDICCOutEvent;
                vst3_event.midiCCOut.channel = midi_event.channel;
                vst3_event.midiCCOut.controlNumber = 0x82;  // >= 0x80 indicates non-CC MIDI
                vst3_event.midiCCOut.value = midi_event.data1;   // LSB
                vst3_event.midiCCOut.value2 = midi_event.data2;  // MSB
                plugin->input_events.addEvent(vst3_event);
                break;
            }

            default:
                // Unknown MIDI event - skip it
                continue;
        }
    }

    return RACK_VST3_OK;
}
