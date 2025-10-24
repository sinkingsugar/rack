cmake_minimum_required(VERSION 3.15)
project(rack_sys VERSION 0.1.0 LANGUAGES CXX)

# C++20 required for aligned allocation and modern C++ features
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Build as static library for easy linking with Rust
add_library(rack_sys STATIC
    src/rack_au.cpp
    src/au_scanner.cpp
    src/au_instance.cpp
    src/au_gui.mm
)

# Public headers
target_include_directories(rack_sys PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

# macOS-specific settings
if(APPLE)
    # Link required frameworks
    target_link_libraries(rack_sys PUBLIC
        "-framework AudioToolbox"
        "-framework CoreAudio"
        "-framework CoreFoundation"
        "-framework CoreAudioKit"
        "-framework AppKit"
    )

    # Enable ARC (Automatic Reference Counting) for Objective-C++ (required for au_gui.mm)
    target_compile_options(rack_sys PRIVATE -fobjc-arc)
endif()

# Compiler warnings
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    target_compile_options(rack_sys PRIVATE
        -Wall
        -Wextra
        -Wpedantic
        -Wno-unused-parameter
    )
endif()

# Install library
install(TARGETS rack_sys
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)

# Install headers
install(DIRECTORY include/
    DESTINATION include
)

# Optional: Enable AddressSanitizer for debugging
option(ENABLE_ASAN "Enable AddressSanitizer for memory error detection" OFF)
if(ENABLE_ASAN)
    add_compile_options(
        $<$<COMPILE_LANGUAGE:CXX,C>:-fsanitize=address>
        $<$<COMPILE_LANGUAGE:CXX,C>:-fno-optimize-sibling-calls>
        $<$<COMPILE_LANGUAGE:CXX,C>:-fsanitize-address-use-after-scope>
        $<$<COMPILE_LANGUAGE:CXX,C>:-fno-omit-frame-pointer>
        $<$<COMPILE_LANGUAGE:CXX,C>:-g>
    )

    if(CMAKE_GENERATOR STREQUAL "Xcode")
        add_link_options(
            -fsanitize=address
            -fno-optimize-sibling-calls
            -fsanitize-address-use-after-scope
            -fno-omit-frame-pointer
            -g
        )
    else()
        add_link_options(
            $<$<COMPILE_LANGUAGE:CXX,C>:-fsanitize=address>
            $<$<COMPILE_LANGUAGE:CXX,C>:-fno-optimize-sibling-calls>
            $<$<COMPILE_LANGUAGE:CXX,C>:-fsanitize-address-use-after-scope>
            $<$<COMPILE_LANGUAGE:CXX,C>:-fno-omit-frame-pointer>
            $<$<COMPILE_LANGUAGE:CXX,C>:-g>
        )
    endif()

    message(STATUS "AddressSanitizer enabled")
endif()

# Optional: Build test executables
option(BUILD_TESTS "Build test executables" OFF)
if(BUILD_TESTS)
    # Scanner test
    add_executable(rack_sys_test_scanner
        test/test_scanner.cpp
    )
    target_link_libraries(rack_sys_test_scanner PRIVATE rack_sys)

    # Instance test
    add_executable(rack_sys_test_instance
        test/test_instance.cpp
    )
    target_link_libraries(rack_sys_test_instance PRIVATE rack_sys)

    # GUI test
    add_executable(rack_sys_test_gui
        test/test_gui.cpp
    )
    target_link_libraries(rack_sys_test_gui PRIVATE rack_sys)
endif()
