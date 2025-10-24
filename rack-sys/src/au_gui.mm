#include "rack_au.h"
#import <AudioToolbox/AudioToolbox.h>
#import <CoreAudioKit/CoreAudioKit.h>
#import <AppKit/AppKit.h>
#import <CoreFoundation/CoreFoundation.h>
#include <cstring>
#include <dispatch/dispatch.h>

// GUI implementation structure
struct RackAUGui {
    AudioComponentInstance audio_unit;
    NSViewController* view_controller;  // For AUv3
    NSView* view;                      // For AUv2 or generic UI
    NSWindow* window;                  // Optional window for standalone display
    bool owns_view_controller;         // Track ownership for cleanup
    bool owns_view;
    char error_message[256];
};

// Callback type for async GUI creation
typedef void (*RackAUGuiCallback)(void* user_data, RackAUGui* gui, int error_code);

// ============================================================================
// Helper Functions
// ============================================================================

// Get parameter count for generic UI
static UInt32 get_parameter_count(AudioComponentInstance audio_unit) {
    UInt32 param_count = 0;
    UInt32 size = 0;

    // Get size of parameter list
    OSStatus status = AudioUnitGetPropertyInfo(
        audio_unit,
        kAudioUnitProperty_ParameterList,
        kAudioUnitScope_Global,
        0,
        &size,
        NULL
    );

    if (status == noErr && size > 0) {
        param_count = size / sizeof(AudioUnitParameterID);
    }

    return param_count;
}

// Get parameter info for generic UI
static bool get_parameter_info(
    AudioComponentInstance audio_unit,
    AudioUnitParameterID param_id,
    AudioUnitParameterInfo* info,
    char* name_buffer,
    size_t name_buffer_size
) {
    UInt32 size = sizeof(AudioUnitParameterInfo);
    OSStatus status = AudioUnitGetProperty(
        audio_unit,
        kAudioUnitProperty_ParameterInfo,
        kAudioUnitScope_Global,
        param_id,
        info,
        &size
    );

    if (status != noErr) {
        return false;
    }

    // Extract parameter name from CFString
    if (info->cfNameString != NULL) {
        CFStringGetCString(
            info->cfNameString,
            name_buffer,
            name_buffer_size,
            kCFStringEncodingUTF8
        );
    } else {
        snprintf(name_buffer, name_buffer_size, "Parameter %u", (unsigned)param_id);
    }

    return true;
}

// Create generic parameter UI using NSStackView
static NSView* create_generic_ui(AudioComponentInstance audio_unit) {
    @autoreleasepool {
        UInt32 param_count = get_parameter_count(audio_unit);

        if (param_count == 0) {
            // Create empty view with message
            NSTextField* label = [[NSTextField alloc] initWithFrame:NSMakeRect(0, 0, 400, 40)];
            [label setStringValue:@"This plugin has no parameters"];
            [label setBezeled:NO];
            [label setDrawsBackground:NO];
            [label setEditable:NO];
            [label setSelectable:NO];
            [label setAlignment:NSTextAlignmentCenter];
            return label;
        }

        // Create vertical stack view
        NSStackView* stackView = [[NSStackView alloc] init];
        [stackView setOrientation:NSUserInterfaceLayoutOrientationVertical];
        [stackView setAlignment:NSLayoutAttributeLeading];
        [stackView setSpacing:10];

        // Get parameter IDs
        UInt32 size = param_count * sizeof(AudioUnitParameterID);
        AudioUnitParameterID* param_ids = (AudioUnitParameterID*)malloc(size);
        OSStatus status = AudioUnitGetProperty(
            audio_unit,
            kAudioUnitProperty_ParameterList,
            kAudioUnitScope_Global,
            0,
            param_ids,
            &size
        );

        if (status != noErr) {
            free(param_ids);
            return stackView;  // Return empty stack view
        }

        // Create UI for each parameter (limit to first 20 for reasonable UI size)
        // TODO (Phase 9): Add scrolling support or make limit configurable
        UInt32 display_count = param_count > 20 ? 20 : param_count;
        for (UInt32 i = 0; i < display_count; i++) {
            AudioUnitParameterID param_id = param_ids[i];
            AudioUnitParameterInfo info;
            char name_buffer[256];

            if (!get_parameter_info(audio_unit, param_id, &info, name_buffer, sizeof(name_buffer))) {
                continue;
            }

            // Create horizontal container for label and slider
            NSStackView* rowView = [[NSStackView alloc] init];
            [rowView setOrientation:NSUserInterfaceLayoutOrientationHorizontal];
            [rowView setSpacing:10];

            // Parameter name label (fixed width)
            NSTextField* nameLabel = [[NSTextField alloc] initWithFrame:NSMakeRect(0, 0, 200, 24)];
            [nameLabel setStringValue:[NSString stringWithUTF8String:name_buffer]];
            [nameLabel setBezeled:NO];
            [nameLabel setDrawsBackground:NO];
            [nameLabel setEditable:NO];
            [nameLabel setSelectable:NO];
            [nameLabel setAlignment:NSTextAlignmentRight];

            // Slider (fixed width)
            NSSlider* slider = [[NSSlider alloc] initWithFrame:NSMakeRect(0, 0, 200, 24)];
            [slider setMinValue:info.minValue];
            [slider setMaxValue:info.maxValue];
            [slider setDoubleValue:info.defaultValue];

            // Get current value from plugin
            AudioUnitParameterValue currentValue = info.defaultValue;
            AudioUnitGetParameter(audio_unit, param_id, kAudioUnitScope_Global, 0, &currentValue);
            [slider setDoubleValue:currentValue];

            // NOTE: Generic UI is read-only (sliders don't update plugin parameters)
            // Most plugins use AUv3/AUv2 custom UIs with bidirectional parameter sync
            // Generic UI is a fallback for plugins with no custom view
            // TODO (Phase 9): Implement slider callbacks with AudioUnitSetParameter
            [slider setTarget:nil];
            [slider setAction:nil];

            // Value label (fixed width)
            NSTextField* valueLabel = [[NSTextField alloc] initWithFrame:NSMakeRect(0, 0, 80, 24)];
            [valueLabel setStringValue:[NSString stringWithFormat:@"%.2f", currentValue]];
            [valueLabel setBezeled:NO];
            [valueLabel setDrawsBackground:NO];
            [valueLabel setEditable:NO];
            [valueLabel setSelectable:NO];
            [valueLabel setAlignment:NSTextAlignmentLeft];

            // Add to row
            [rowView addView:nameLabel inGravity:NSStackViewGravityLeading];
            [rowView addView:slider inGravity:NSStackViewGravityLeading];
            [rowView addView:valueLabel inGravity:NSStackViewGravityLeading];

            // Add row to stack
            [stackView addView:rowView inGravity:NSStackViewGravityTop];
        }

        free(param_ids);

        // Set stack view size
        NSSize contentSize = [stackView fittingSize];
        [stackView setFrameSize:contentSize];

        return stackView;
    }
}

// ============================================================================
// AUv3 GUI Loading (Asynchronous)
// ============================================================================

static void try_load_auv3_gui(
    AudioComponentInstance audio_unit,
    void (^completion)(AUViewControllerBase* viewController)
) {
    @autoreleasepool {
        // Get the AudioComponent from the instance
        AudioComponent component = AudioComponentInstanceGetComponent(audio_unit);
        if (component == NULL) {
            completion(nil);
            return;
        }

        // Get component description to instantiate AUv3
        AudioComponentDescription desc;
        if (AudioComponentGetDescription(component, &desc) != noErr) {
            completion(nil);
            return;
        }

        // Try to instantiate as AUv3 (asynchronously)
        [AUAudioUnit instantiateWithComponentDescription:desc
                                                  options:0
                                        completionHandler:^(AUAudioUnit* _Nullable auAudioUnit, NSError* _Nullable error) {
            if (error != nil || auAudioUnit == nil) {
                // Not an AUv3 plugin or instantiation failed
                completion(nil);
                return;
            }

            // Request view controller asynchronously
            [auAudioUnit requestViewControllerWithCompletionHandler:^(AUViewControllerBase* _Nullable viewController) {
                // viewController will be nil if plugin has no GUI
                completion(viewController);
            }];
        }];
    }
}

// ============================================================================
// AUv2 GUI Loading (Synchronous)
// ============================================================================

static NSView* try_load_auv2_gui(AudioComponentInstance audio_unit) {
    @autoreleasepool {
        AudioUnitCocoaViewInfo viewInfo;
        UInt32 dataSize = sizeof(AudioUnitCocoaViewInfo);

        OSStatus status = AudioUnitGetProperty(
            audio_unit,
            kAudioUnitProperty_CocoaUI,
            kAudioUnitScope_Global,
            0,
            &viewInfo,
            &dataSize
        );

        if (status != noErr || viewInfo.mCocoaAUViewBundleLocation == NULL) {
            return nil;
        }

        // Convert CFURLRef to NSURL
        // AudioUnitGetProperty returns owned CF objects (Create Rule)
        // __bridge doesn't transfer ownership to ARC, so manual CFRelease required
        NSURL* bundleURL = (__bridge NSURL*)viewInfo.mCocoaAUViewBundleLocation;
        NSBundle* viewBundle = [NSBundle bundleWithURL:bundleURL];

        if (viewBundle == nil) {
            CFRelease(viewInfo.mCocoaAUViewBundleLocation);
            if (viewInfo.mCocoaAUViewClass[0] != NULL) {
                CFRelease(viewInfo.mCocoaAUViewClass[0]);
            }
            return nil;
        }

        // Get view class name
        NSString* viewClassName = (__bridge NSString*)viewInfo.mCocoaAUViewClass[0];
        Class viewClass = [viewBundle classNamed:viewClassName];

        if (viewClass == nil) {
            CFRelease(viewInfo.mCocoaAUViewBundleLocation);
            CFRelease(viewInfo.mCocoaAUViewClass[0]);
            return nil;
        }

        // Create view instance
        // The view class should have an initWithAudioUnit: method
        NSView* auView = nil;
        if ([viewClass instancesRespondToSelector:@selector(initWithAudioUnit:)]) {
            // Use NSInvocation to avoid performSelector warning
            SEL selector = @selector(initWithAudioUnit:);
            NSMethodSignature *signature = [viewClass instanceMethodSignatureForSelector:selector];
            NSInvocation *invocation = [NSInvocation invocationWithMethodSignature:signature];
            [invocation setSelector:selector];
            id instance = [[viewClass alloc] init];
            [invocation setTarget:instance];
            [invocation setArgument:&audio_unit atIndex:2]; // arg 0 is self, arg 1 is _cmd
            [invocation invoke];
            [invocation getReturnValue:&auView];
        }

        // Clean up CoreFoundation objects
        CFRelease(viewInfo.mCocoaAUViewBundleLocation);
        CFRelease(viewInfo.mCocoaAUViewClass[0]);

        return auView;
    }
}

// Helper to get AudioComponentInstance from RackAUPlugin
// We need this because RackAUPlugin is opaque in this file
extern "C" AudioComponentInstance rack_au_plugin_get_audio_unit(RackAUPlugin* plugin);

// ============================================================================
// Public C API
// ============================================================================

extern "C" {

// Create GUI asynchronously
// Tries AUv3 (modern) → AUv2 (legacy) → generic UI (fallback)
// Callback is invoked on main thread when GUI is ready
void rack_au_gui_create_async(
    RackAUPlugin* plugin,
    RackAUGuiCallback callback,
    void* user_data
) {
    if (!plugin || !callback) {
        if (callback) {
            callback(user_data, NULL, RACK_AU_ERROR_INVALID_PARAM);
        }
        return;
    }

    // Get AudioComponentInstance from plugin
    AudioComponentInstance audio_unit = rack_au_plugin_get_audio_unit(plugin);
    if (audio_unit == NULL) {
        callback(user_data, NULL, RACK_AU_ERROR_INVALID_PARAM);
        return;
    }

    // Ensure we're on main thread for GUI operations
    dispatch_async(dispatch_get_main_queue(), ^{
        @autoreleasepool {
            // Try AUv3 GUI first (asynchronous)
            try_load_auv3_gui(audio_unit, ^(AUViewControllerBase* viewController) {
                if (viewController != nil) {
                    // AUv3 succeeded
                    RackAUGui* gui = new RackAUGui();
                    gui->audio_unit = audio_unit;
                    gui->view_controller = viewController;
                    gui->view = viewController.view;
                    gui->window = nil;
                    gui->owns_view_controller = true;
                    gui->owns_view = false;  // View is owned by view controller
                    gui->error_message[0] = '\0';

                    callback(user_data, gui, RACK_AU_OK);
                } else {
                    // AUv3 failed, try AUv2
                    NSView* auv2_view = try_load_auv2_gui(audio_unit);

                    if (auv2_view != nil) {
                        // AUv2 succeeded
                        RackAUGui* gui = new RackAUGui();
                        gui->audio_unit = audio_unit;
                        gui->view_controller = nil;
                        gui->view = auv2_view;
                        gui->window = nil;
                        gui->owns_view_controller = false;
                        gui->owns_view = true;
                        gui->error_message[0] = '\0';

                        callback(user_data, gui, RACK_AU_OK);
                    } else {
                        // Both AUv3 and AUv2 failed, create generic parameter UI as fallback
                        NSView* generic_view = create_generic_ui(audio_unit);

                        RackAUGui* gui = new RackAUGui();
                        gui->audio_unit = audio_unit;
                        gui->view_controller = nil;
                        gui->view = generic_view;
                        gui->window = nil;
                        gui->owns_view_controller = false;
                        gui->owns_view = true;
                        gui->error_message[0] = '\0';

                        callback(user_data, gui, RACK_AU_OK);
                    }
                }
            });
        }
    });
}

// Destroy GUI and clean up resources
// IMPORTANT: gui pointer becomes invalid immediately after this call
// Cleanup happens asynchronously on main thread to avoid deadlocks
// Rust Drop impl ensures this is safe (ownership transferred)
void rack_au_gui_destroy(RackAUGui* gui) {
    if (!gui) {
        return;
    }

    dispatch_async(dispatch_get_main_queue(), ^{
        @autoreleasepool {
            // Close window if we created one
            if (gui->window != nil) {
                [gui->window close];
                gui->window = nil;
            }

            // Clean up view controller
            if (gui->owns_view_controller && gui->view_controller != nil) {
                gui->view_controller = nil;  // ARC will handle cleanup
            }

            // Clean up view
            if (gui->owns_view && gui->view != nil) {
                [gui->view removeFromSuperview];
                gui->view = nil;  // ARC will handle cleanup
            }

            delete gui;
        }
    });
}

// Get native NSView pointer for embedding in host UI
// Returns void* that can be cast to NSView* in Objective-C code
void* rack_au_gui_get_view(RackAUGui* gui) {
    if (!gui) {
        return NULL;
    }

    return (__bridge void*)gui->view;
}

// Get view size
int rack_au_gui_get_size(RackAUGui* gui, float* width, float* height) {
    if (!gui || !gui->view || !width || !height) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    __block NSSize size;
    dispatch_sync(dispatch_get_main_queue(), ^{
        size = [gui->view frame].size;
    });

    *width = size.width;
    *height = size.height;

    return RACK_AU_OK;
}

// Create and show window with GUI
int rack_au_gui_show_window(RackAUGui* gui, const char* title) {
    if (!gui || !gui->view) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    dispatch_async(dispatch_get_main_queue(), ^{
        @autoreleasepool {
            // Get view size
            NSSize viewSize = [gui->view frame].size;

            // Create window if needed
            if (gui->window == nil) {
                NSRect frame = NSMakeRect(100, 100, viewSize.width, viewSize.height);
                gui->window = [[NSWindow alloc] initWithContentRect:frame
                                                          styleMask:(NSWindowStyleMaskTitled |
                                                                   NSWindowStyleMaskClosable |
                                                                   NSWindowStyleMaskMiniaturizable)
                                                            backing:NSBackingStoreBuffered
                                                              defer:NO];

                [gui->window setContentView:gui->view];

                if (title != NULL) {
                    [gui->window setTitle:[NSString stringWithUTF8String:title]];
                } else {
                    [gui->window setTitle:@"AudioUnit GUI"];
                }
            }

            [gui->window makeKeyAndOrderFront:nil];
            [gui->window center];
        }
    });

    return RACK_AU_OK;
}

// Hide window
int rack_au_gui_hide_window(RackAUGui* gui) {
    if (!gui) {
        return RACK_AU_ERROR_INVALID_PARAM;
    }

    dispatch_async(dispatch_get_main_queue(), ^{
        if (gui->window != nil) {
            [gui->window orderOut:nil];
        }
    });

    return RACK_AU_OK;
}

} // extern "C"
