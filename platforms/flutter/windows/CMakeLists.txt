# Merman plugin for Windows. Rendering happens via Dart FFI; this target only
# registers the Flutter plugin and copies the bundled native library.
cmake_minimum_required(VERSION 3.14)
project(merman LANGUAGES CXX)

set(PLUGIN_NAME "merman_plugin")

list(APPEND PLUGIN_SOURCES
  "merman_flutter_plugin.cpp"
  "merman_flutter_plugin_c_api.cpp"
)

add_library(${PLUGIN_NAME} SHARED
  ${PLUGIN_SOURCES}
)

apply_standard_settings(${PLUGIN_NAME})
set_target_properties(${PLUGIN_NAME} PROPERTIES
  CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
target_include_directories(${PLUGIN_NAME} PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter flutter_wrapper_plugin)

set(merman_bundled_libraries
  "${CMAKE_CURRENT_SOURCE_DIR}/merman_ffi.dll"
  PARENT_SCOPE
)

add_custom_command(TARGET ${PLUGIN_NAME} POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
    "${CMAKE_CURRENT_SOURCE_DIR}/merman_ffi.dll"
    "$<TARGET_FILE_DIR:${PLUGIN_NAME}>/merman_ffi.dll"
)
