#ifndef RACK_AU_H
#define RACK_AU_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>

// Opaque types
typedef struct RackAUScanner RackAUScanner;
typedef struct RackAUPlugin RackAUPlugin;

// Plugin type enum
typedef enum {
    RACK_AU_TYPE_EFFECT = 0,
    RACK_AU_TYPE_INSTRUMENT = 1,
    RACK_AU_TYPE_MIXER = 2,
    RACK_AU_TYPE_FORMAT_CONVERTER = 3,
    RACK_AU_TYPE_OTHER = 4,
} RackAUPluginType;

// Plugin info struct (passed to Rust)
typedef struct {
    char name[256];
    char manufacturer[256];
    char path[1024];
    char unique_id[64];
    uint32_t version;
    RackAUPluginType plugin_type;
} RackAUPluginInfo;

// Error codes (0 = success, negative = error)
#define RACK_AU_OK 0
#define RACK_AU_ERROR_GENERIC -1
#define RACK_AU_ERROR_NOT_FOUND -2
#define RACK_AU_ERROR_INVALID_PARAM -3
#define RACK_AU_ERROR_NOT_INITIALIZED -4
#define RACK_AU_ERROR_AUDIO_UNIT -1000  // Base for AudioUnit OSStatus errors

// ============================================================================
// Scanner API
// ============================================================================

// Create a new scanner
RackAUScanner* rack_au_scanner_new(void);

// Free scanner
void rack_au_scanner_free(RackAUScanner* scanner);

// Scan for plugins
// Returns number of plugins found (or would be found), or negative error code
//
// Two-pass usage pattern (recommended):
//   1. count = rack_au_scanner_scan(scanner, NULL, 0);  // Get total count
//   2. rack_au_scanner_scan(scanner, array, count);     // Fill array
//
// If plugins is NULL: Only counts plugins, does not extract details
// If plugins is not NULL: Fills array up to max_plugins
//
// IMPORTANT: Return value may exceed max_plugins if more plugins exist.
//            Compare return value with max_plugins to detect truncation.
//
// plugins: output array (allocated by caller), or NULL to get count only
// max_plugins: size of output array (ignored if plugins is NULL)
int rack_au_scanner_scan(RackAUScanner* scanner, RackAUPluginInfo* plugins, size_t max_plugins);

// ============================================================================
// Plugin Instance API
// ============================================================================

// Create a new plugin instance from unique_id
RackAUPlugin* rack_au_plugin_new(const char* unique_id);

// Free plugin instance
void rack_au_plugin_free(RackAUPlugin* plugin);

// Initialize plugin
// Returns 0 on success, negative error code on failure
int rack_au_plugin_initialize(RackAUPlugin* plugin, double sample_rate, uint32_t max_block_size);

// Check if plugin is initialized
int rack_au_plugin_is_initialized(RackAUPlugin* plugin);

// Process audio
// input: input buffer (interleaved stereo)
// output: output buffer (interleaved stereo)
// frames: number of frames to process
// Returns 0 on success, negative error code on failure
int rack_au_plugin_process(RackAUPlugin* plugin, const float* input, float* output, uint32_t frames);

// Get parameter count
int rack_au_plugin_parameter_count(RackAUPlugin* plugin);

// Get parameter value (normalized 0.0 to 1.0)
// Returns 0 on success, negative error code on failure
int rack_au_plugin_get_parameter(RackAUPlugin* plugin, uint32_t index, float* value);

// Set parameter value (normalized 0.0 to 1.0)
// Returns 0 on success, negative error code on failure
int rack_au_plugin_set_parameter(RackAUPlugin* plugin, uint32_t index, float value);

// Get parameter info
// name: output buffer for parameter name (allocated by caller)
// name_size: size of name buffer
// Returns 0 on success, negative error code on failure
int rack_au_plugin_parameter_info(
    RackAUPlugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    float* min,
    float* max,
    float* default_value
);

#ifdef __cplusplus
}
#endif

#endif // RACK_AU_H
