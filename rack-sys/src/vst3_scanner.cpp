#include "rack_vst3.h"
#include "public.sdk/source/vst/hosting/module.h"
#include "public.sdk/source/vst/hosting/plugprovider.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivstcomponent.h"

#include <vector>
#include <string>
#include <algorithm>
#include <cstring>

#if defined(__APPLE__)
    #include <CoreFoundation/CoreFoundation.h>
#elif defined(_WIN32)
    #include <windows.h>
    #include <shlobj.h>
#else
    #include <unistd.h>
    #include <pwd.h>
#endif

using namespace VST3;
using namespace Steinberg;
using namespace Steinberg::Vst;

// Internal scanner state
struct RackVST3Scanner {
    std::vector<std::string> search_paths;
};

// Helper: Get default VST3 plugin paths for the current platform
static std::vector<std::string> get_default_vst3_paths() {
    std::vector<std::string> paths;

#if defined(__APPLE__)
    // macOS paths
    paths.push_back("/Library/Audio/Plug-Ins/VST3");

    // User library
    const char* home = getenv("HOME");
    if (home) {
        std::string user_path = std::string(home) + "/Library/Audio/Plug-Ins/VST3";
        paths.push_back(user_path);
    }

#elif defined(_WIN32)
    // Windows paths
    char common_files[MAX_PATH];
    if (SUCCEEDED(SHGetFolderPathA(NULL, CSIDL_PROGRAM_FILES_COMMON, NULL, 0, common_files))) {
        std::string path = std::string(common_files) + "\\VST3";
        paths.push_back(path);
    }

#else
    // Linux paths
    paths.push_back("/usr/lib/vst3");
    paths.push_back("/usr/local/lib/vst3");

    // User home
    const char* home = getenv("HOME");
    if (home) {
        std::string user_path = std::string(home) + "/.vst3";
        paths.push_back(user_path);
    }
#endif

    return paths;
}

// Helper: Convert VST3 UID to hex string
static std::string uid_to_string(const VST3::UID& uid) {
    return uid.toString();
}

// Helper: Determine plugin type from subcategories
static RackVST3PluginType determine_plugin_type(const std::string& subcategories) {
    if (subcategories.find("Instrument") != std::string::npos) {
        return RACK_VST3_TYPE_INSTRUMENT;
    } else if (subcategories.find("Analyzer") != std::string::npos) {
        return RACK_VST3_TYPE_ANALYZER;
    } else if (subcategories.find("Spatial") != std::string::npos) {
        return RACK_VST3_TYPE_SPATIAL;
    } else if (subcategories.find("Fx") != std::string::npos) {
        return RACK_VST3_TYPE_EFFECT;
    }
    return RACK_VST3_TYPE_OTHER;
}

// ============================================================================
// Scanner Implementation
// ============================================================================

RackVST3Scanner* rack_vst3_scanner_new(void) {
    return new(std::nothrow) RackVST3Scanner();
}

void rack_vst3_scanner_free(RackVST3Scanner* scanner) {
    delete scanner;
}

int rack_vst3_scanner_add_path(RackVST3Scanner* scanner, const char* path) {
    if (!scanner || !path) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    scanner->search_paths.push_back(path);
    return RACK_VST3_OK;
}

int rack_vst3_scanner_add_default_paths(RackVST3Scanner* scanner) {
    if (!scanner) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    auto paths = get_default_vst3_paths();
    scanner->search_paths.insert(scanner->search_paths.end(), paths.begin(), paths.end());
    return RACK_VST3_OK;
}

int rack_vst3_scanner_scan(RackVST3Scanner* scanner, RackVST3PluginInfo* plugins, size_t max_plugins) {
    if (!scanner) {
        return RACK_VST3_ERROR_INVALID_PARAM;
    }

    bool count_only = (plugins == nullptr);
    size_t count = 0;

    // If no paths added, use default paths
    std::vector<std::string> paths_to_scan = scanner->search_paths;
    if (paths_to_scan.empty()) {
        paths_to_scan = get_default_vst3_paths();
    }

    // Get all module paths using VST3 SDK's discovery
    // Note: This gets system-wide VST3 plugins
    auto module_paths = Hosting::Module::getModulePaths();

    // TODO: Add support for custom search paths
    // For now, we rely on the SDK's module discovery which finds plugins in standard locations
    (void)paths_to_scan;  // Suppress unused variable warning

    // Scan all found modules
    for (const auto& module_path : module_paths) {
        std::string error_description;
        auto module = Hosting::Module::create(module_path, error_description);

        if (!module) {
            // Failed to load module, skip it
            continue;
        }

        const auto& factory = module->getFactory();
        auto class_infos = factory.classInfos();

        // Enumerate all classes in this module
        for (const auto& class_info : class_infos) {
            // Only process audio effect classes
            if (class_info.category() != kVstAudioEffectClass) {
                continue;
            }

            // If we're just counting, increment and continue
            if (count_only) {
                count++;
                continue;
            }

            // If array is full, continue counting but don't fill
            if (count >= max_plugins) {
                count++;
                continue;
            }

            // Fill in plugin info
            RackVST3PluginInfo& info = plugins[count];

            // Name
            std::string name = class_info.name();
            strncpy(info.name, name.c_str(), sizeof(info.name) - 1);
            info.name[sizeof(info.name) - 1] = '\0';

            // Manufacturer
            std::string vendor = class_info.vendor();
            if (vendor.empty()) {
                vendor = factory.info().vendor();
            }
            strncpy(info.manufacturer, vendor.c_str(), sizeof(info.manufacturer) - 1);
            info.manufacturer[sizeof(info.manufacturer) - 1] = '\0';

            // Path (full path to the .vst3 bundle/folder)
            strncpy(info.path, module_path.c_str(), sizeof(info.path) - 1);
            info.path[sizeof(info.path) - 1] = '\0';

            // Unique ID (UID as hex string)
            std::string uid_str = uid_to_string(class_info.ID());
            strncpy(info.unique_id, uid_str.c_str(), sizeof(info.unique_id) - 1);
            info.unique_id[sizeof(info.unique_id) - 1] = '\0';

            // Version
            std::string version = class_info.version();
            // Parse version string (e.g., "1.0.0") to uint32_t
            // For simplicity, we'll just use 0 for now
            // TODO: Implement proper version parsing
            info.version = 0;

            // Type (from subcategories)
            std::string subcategories = class_info.subCategoriesString();
            info.plugin_type = determine_plugin_type(subcategories);

            // Category (subcategories string)
            strncpy(info.category, subcategories.c_str(), sizeof(info.category) - 1);
            info.category[sizeof(info.category) - 1] = '\0';

            count++;
        }
    }

    return static_cast<int>(count);
}
