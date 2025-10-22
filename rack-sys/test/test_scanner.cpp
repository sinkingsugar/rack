#include "rack_au.h"
#include <iostream>
#include <iomanip>

int main() {
    std::cout << "AudioUnit Scanner Test\n";
    std::cout << "======================\n\n";

    // Create scanner
    RackAUScanner* scanner = rack_au_scanner_new();
    if (!scanner) {
        std::cerr << "Failed to create scanner\n";
        return 1;
    }

    // Prepare buffer for results
    const size_t MAX_PLUGINS = 256;
    RackAUPluginInfo plugins[MAX_PLUGINS];

    // Scan for plugins
    std::cout << "Scanning for AudioUnit plugins...\n\n";
    int count = rack_au_scanner_scan(scanner, plugins, MAX_PLUGINS);

    if (count < 0) {
        std::cerr << "Error scanning: " << count << "\n";
        rack_au_scanner_free(scanner);
        return 1;
    }

    std::cout << "Found " << count << " plugin(s):\n\n";

    // Print results
    for (int i = 0; i < count; i++) {
        const RackAUPluginInfo& info = plugins[i];

        std::cout << (i + 1) << ". " << info.name << "\n";
        std::cout << "   Manufacturer: " << info.manufacturer << "\n";
        std::cout << "   Path: " << info.path << "\n";
        std::cout << "   ID: " << info.unique_id << "\n";
        std::cout << "   Version: 0x" << std::hex << info.version << std::dec << "\n";

        std::cout << "   Type: ";
        switch (info.plugin_type) {
            case RACK_AU_TYPE_EFFECT:
                std::cout << "Effect";
                break;
            case RACK_AU_TYPE_INSTRUMENT:
                std::cout << "Instrument";
                break;
            case RACK_AU_TYPE_MIXER:
                std::cout << "Mixer";
                break;
            case RACK_AU_TYPE_FORMAT_CONVERTER:
                std::cout << "Format Converter";
                break;
            case RACK_AU_TYPE_OTHER:
                std::cout << "Other";
                break;
            default:
                std::cout << "Unknown";
        }
        std::cout << "\n\n";
    }

    // Cleanup
    rack_au_scanner_free(scanner);

    std::cout << "Test completed successfully!\n";
    return 0;
}
