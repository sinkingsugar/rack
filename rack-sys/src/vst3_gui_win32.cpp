#ifdef _WIN32

#include "rack_vst3.h"
#include "pluginterfaces/gui/iplugview.h"
#include "pluginterfaces/gui/iplugviewcontentscalesupport.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include <windows.h>
#include <cstring>

using namespace Steinberg;

// Forward declaration — defined in vst3_instance.cpp
extern "C" Steinberg::Vst::IEditController* rack_vst3_plugin_get_controller(RackVST3Plugin* plugin);

// ============================================================================
// IPlugFrame implementation
// ============================================================================

class RackPlugFrame : public IPlugFrame {
public:
    RackPlugFrame() = default;

    void set_hwnd(HWND hwnd) { hwnd_ = hwnd; }

    // IPlugFrame
    tresult PLUGIN_API resizeView(IPlugView* view, ViewRect* new_size) override {
        if (!view || !new_size || !hwnd_) return kInvalidArgument;
        if (in_resize_) return kResultOk; // Recursion guard
        in_resize_ = true;

        int32_t w = new_size->right - new_size->left;
        int32_t h = new_size->bottom - new_size->top;

        RECT rc = {0, 0, static_cast<LONG>(w), static_cast<LONG>(h)};
        DWORD style = static_cast<DWORD>(GetWindowLongPtr(hwnd_, GWL_STYLE));
        DWORD ex_style = static_cast<DWORD>(GetWindowLongPtr(hwnd_, GWL_EXSTYLE));
        AdjustWindowRectEx(&rc, style, FALSE, ex_style);

        SetWindowPos(hwnd_, nullptr, 0, 0,
                     rc.right - rc.left, rc.bottom - rc.top,
                     SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);

        view->onSize(new_size);
        in_resize_ = false;
        return kResultOk;
    }

    // FUnknown — fixed refcount (prevents premature release by plugin)
    tresult PLUGIN_API queryInterface(const TUID iid, void** obj) override {
        if (FUnknownPrivate::iidEqual(iid, IPlugFrame::iid) ||
            FUnknownPrivate::iidEqual(iid, FUnknown::iid)) {
            *obj = static_cast<IPlugFrame*>(this);
            return kResultOk;
        }
        *obj = nullptr;
        return kNoInterface;
    }
    uint32 PLUGIN_API addRef() override { return 1000; }
    uint32 PLUGIN_API release() override { return 1000; }

private:
    HWND hwnd_ = nullptr;
    bool in_resize_ = false;
};

// ============================================================================
// RackVST3Gui struct
// ============================================================================

struct RackVST3Gui {
    IPtr<IPlugView> view;
    RackPlugFrame plug_frame;
    HWND hwnd = nullptr;
    bool is_resizable = false;
    int32_t width = 0;
    int32_t height = 0;
    RackVST3GuiCloseCallback close_callback = nullptr;
    void* close_user_data = nullptr;
};

// ============================================================================
// Win32 window class + WndProc
// ============================================================================

static const wchar_t* WINDOW_CLASS_NAME = L"RackVST3EditorWindow";
static bool g_window_class_registered = false;

static LRESULT CALLBACK EditorWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    auto* gui = reinterpret_cast<RackVST3Gui*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));

    switch (msg) {
        case WM_CLOSE: {
            if (gui && gui->view) {
                gui->view->removed();
                gui->view->setFrame(nullptr);
            }
            ShowWindow(hwnd, SW_HIDE);
            // Fire close callback
            if (gui && gui->close_callback) {
                gui->close_callback(gui->close_user_data);
            }
            return 0; // Don't destroy — we manage lifecycle via rack_vst3_gui_destroy
        }

        case WM_SIZE: {
            if (!gui || !gui->view) break;
            RECT rc;
            GetClientRect(hwnd, &rc);
            int32_t w = rc.right - rc.left;
            int32_t h = rc.bottom - rc.top;
            if (w > 0 && h > 0 && (w != gui->width || h != gui->height)) {
                gui->width = w;
                gui->height = h;
                ViewRect vr = {0, 0, w, h};
                gui->view->onSize(&vr);
            }
            return 0;
        }

        case WM_SIZING: {
            if (!gui || !gui->view) break;
            auto* rc = reinterpret_cast<RECT*>(lParam);
            // Get client area from proposed window rect
            RECT client = *rc;
            DWORD style = static_cast<DWORD>(GetWindowLongPtr(hwnd, GWL_STYLE));
            DWORD ex_style = static_cast<DWORD>(GetWindowLongPtr(hwnd, GWL_EXSTYLE));
            // Invert AdjustWindowRectEx to get client size
            RECT border = {0, 0, 0, 0};
            AdjustWindowRectEx(&border, style, FALSE, ex_style);
            client.left -= border.left;
            client.top -= border.top;
            client.right -= border.right;
            client.bottom -= border.bottom;

            int32_t cw = client.right - client.left;
            int32_t ch = client.bottom - client.top;
            ViewRect vr = {0, 0, cw, ch};
            if (gui->view->checkSizeConstraint(&vr) == kResultOk) {
                int32_t new_cw = vr.right - vr.left;
                int32_t new_ch = vr.bottom - vr.top;
                if (new_cw != cw || new_ch != ch) {
                    RECT adjusted = {0, 0, static_cast<LONG>(new_cw), static_cast<LONG>(new_ch)};
                    AdjustWindowRectEx(&adjusted, style, FALSE, ex_style);
                    // Keep the anchor edge based on which border the user is dragging
                    switch (wParam) {
                        case WMSZ_LEFT:
                        case WMSZ_TOPLEFT:
                        case WMSZ_BOTTOMLEFT:
                            rc->left = rc->right - (adjusted.right - adjusted.left);
                            break;
                        default:
                            rc->right = rc->left + (adjusted.right - adjusted.left);
                            break;
                    }
                    switch (wParam) {
                        case WMSZ_TOP:
                        case WMSZ_TOPLEFT:
                        case WMSZ_TOPRIGHT:
                            rc->top = rc->bottom - (adjusted.bottom - adjusted.top);
                            break;
                        default:
                            rc->bottom = rc->top + (adjusted.bottom - adjusted.top);
                            break;
                    }
                }
            }
            return TRUE;
        }

        case WM_DPICHANGED: {
            if (!gui || !gui->view) break;
            float new_dpi = static_cast<float>(HIWORD(wParam));
            float scale = new_dpi / 96.0f;

            // Notify plugin of scale change
            FUnknownPtr<IPlugViewContentScaleSupport> css(gui->view);
            if (css) {
                css->setContentScaleFactor(scale);
            }

            // Reposition to the suggested rect from Windows
            auto* suggested = reinterpret_cast<RECT*>(lParam);
            SetWindowPos(hwnd, nullptr,
                         suggested->left, suggested->top,
                         suggested->right - suggested->left,
                         suggested->bottom - suggested->top,
                         SWP_NOZORDER | SWP_NOACTIVATE);
            return 0;
        }

        case WM_ERASEBKGND:
            return TRUE; // Plugin draws everything

        case WM_PAINT: {
            PAINTSTRUCT ps;
            BeginPaint(hwnd, &ps);
            EndPaint(hwnd, &ps);
            return 0;
        }

        default:
            break;
    }

    return DefWindowProc(hwnd, msg, wParam, lParam);
}

static void ensure_window_class() {
    if (g_window_class_registered) return;

    WNDCLASSEXW wc = {};
    wc.cbSize = sizeof(WNDCLASSEXW);
    wc.style = CS_DBLCLKS;
    wc.lpfnWndProc = EditorWndProc;
    wc.hInstance = GetModuleHandle(nullptr);
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    wc.hbrBackground = nullptr;
    wc.lpszClassName = WINDOW_CLASS_NAME;

    RegisterClassExW(&wc);
    g_window_class_registered = true;
}

// ============================================================================
// C API implementation
// ============================================================================

extern "C" {

int rack_vst3_gui_has_editor(RackVST3Plugin* plugin) {
    auto* controller = rack_vst3_plugin_get_controller(plugin);
    if (!controller) return 0;

    IPtr<IPlugView> view(controller->createView("editor"), false);
    if (!view) return 0;

    if (view->isPlatformTypeSupported(kPlatformTypeHWND) != kResultOk) {
        return 0;
    }

    return 1;
}

RackVST3Gui* rack_vst3_gui_create(RackVST3Plugin* plugin) {
    auto* controller = rack_vst3_plugin_get_controller(plugin);
    if (!controller) return nullptr;

    IPtr<IPlugView> view(controller->createView("editor"), false);
    if (!view) return nullptr;

    if (view->isPlatformTypeSupported(kPlatformTypeHWND) != kResultOk) {
        return nullptr;
    }

    // Query initial size
    ViewRect rect = {};
    if (view->getSize(&rect) != kResultOk) {
        rect = {0, 0, 640, 480}; // Fallback
    }

    auto* gui = new RackVST3Gui();
    gui->view = std::move(view);
    gui->width = rect.right - rect.left;
    gui->height = rect.bottom - rect.top;
    gui->is_resizable = (gui->view->canResize() == kResultTrue);

    // Set plug frame so the plugin can request resizes
    gui->view->setFrame(&gui->plug_frame);

    return gui;
}

void rack_vst3_gui_destroy(RackVST3Gui* gui) {
    if (!gui) return;

    if (gui->hwnd) {
        if (gui->view) {
            gui->view->removed();
            gui->view->setFrame(nullptr);
        }
        DestroyWindow(gui->hwnd);
        gui->hwnd = nullptr;
    } else if (gui->view) {
        gui->view->setFrame(nullptr);
    }

    delete gui;
}

int rack_vst3_gui_get_size(RackVST3Gui* gui, int32_t* width, int32_t* height) {
    if (!gui || !width || !height) return RACK_VST3_ERROR_INVALID_PARAM;
    *width = gui->width;
    *height = gui->height;
    return RACK_VST3_OK;
}

int rack_vst3_gui_show_window(RackVST3Gui* gui, const char* title) {
    if (!gui || !gui->view) return RACK_VST3_ERROR_INVALID_PARAM;

    // If already shown, just bring to front
    if (gui->hwnd && IsWindowVisible(gui->hwnd)) {
        SetForegroundWindow(gui->hwnd);
        return RACK_VST3_OK;
    }

    ensure_window_class();

    // Convert title to wide string
    int title_len = MultiByteToWideChar(CP_UTF8, 0, title ? title : "Plugin Editor", -1, nullptr, 0);
    auto* wide_title = new wchar_t[title_len];
    MultiByteToWideChar(CP_UTF8, 0, title ? title : "Plugin Editor", -1, wide_title, title_len);

    // Window style
    DWORD style = WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    if (gui->is_resizable) {
        style |= WS_SIZEBOX;
    }
    DWORD ex_style = WS_EX_APPWINDOW;

    // Calculate window rect from desired client area
    RECT rc = {0, 0, static_cast<LONG>(gui->width), static_cast<LONG>(gui->height)};
    AdjustWindowRectEx(&rc, style, FALSE, ex_style);

    if (!gui->hwnd) {
        gui->hwnd = CreateWindowExW(
            ex_style,
            WINDOW_CLASS_NAME,
            wide_title,
            style,
            CW_USEDEFAULT, CW_USEDEFAULT,
            rc.right - rc.left, rc.bottom - rc.top,
            nullptr, nullptr,
            GetModuleHandle(nullptr),
            nullptr
        );
        delete[] wide_title;

        if (!gui->hwnd) return RACK_VST3_ERROR_GENERIC;

        SetWindowLongPtr(gui->hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(gui));
        gui->plug_frame.set_hwnd(gui->hwnd);
    } else {
        SetWindowTextW(gui->hwnd, wide_title);
        delete[] wide_title;
    }

    // Set DPI scale factor if supported
    {
        UINT dpi = 96;
        // GetDpiForWindow requires Windows 10 1607+
        using GetDpiForWindowFunc = UINT(WINAPI*)(HWND);
        HMODULE user32 = GetModuleHandleW(L"user32.dll");
        if (user32) {
            auto fn = reinterpret_cast<GetDpiForWindowFunc>(
                GetProcAddress(user32, "GetDpiForWindow"));
            if (fn) {
                dpi = fn(gui->hwnd);
            }
        }

        float scale = static_cast<float>(dpi) / 96.0f;
        FUnknownPtr<IPlugViewContentScaleSupport> css(gui->view);
        if (css) {
            css->setContentScaleFactor(scale);
        }
    }

    // Attach the plugin view to the window
    if (gui->view->attached(gui->hwnd, kPlatformTypeHWND) != kResultOk) {
        DestroyWindow(gui->hwnd);
        gui->hwnd = nullptr;
        return RACK_VST3_ERROR_GENERIC;
    }

    ShowWindow(gui->hwnd, SW_SHOW);
    UpdateWindow(gui->hwnd);

    return RACK_VST3_OK;
}

int rack_vst3_gui_hide_window(RackVST3Gui* gui) {
    if (!gui) return RACK_VST3_ERROR_INVALID_PARAM;

    if (gui->hwnd && gui->view) {
        gui->view->removed();
        ShowWindow(gui->hwnd, SW_HIDE);
    }

    return RACK_VST3_OK;
}

void rack_vst3_gui_set_close_callback(RackVST3Gui* gui, RackVST3GuiCloseCallback callback, void* user_data) {
    if (!gui) return;
    gui->close_callback = callback;
    gui->close_user_data = user_data;
}

} // extern "C"

#endif // _WIN32
