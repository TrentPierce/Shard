cmake_minimum_required(VERSION 3.18)
project(shard_engine LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# Disable llama.cpp extras to speed up build
set(LLAMA_BUILD_EXAMPLES OFF CACHE BOOL "llama: build examples" FORCE)
set(LLAMA_BUILD_TESTS OFF CACHE BOOL "llama: build tests" FORCE)
set(LLAMA_BUILD_SERVER OFF CACHE BOOL "llama: build server example" FORCE)

# Build llama.cpp as a dependency
add_subdirectory(../llama.cpp llama.cpp EXCLUDE_FROM_ALL)

add_library(shard_engine SHARED
  shard_bridge.cpp
)

target_compile_definitions(shard_engine PRIVATE SHARD_ENGINE_BUILD)

target_include_directories(shard_engine PUBLIC
  ${CMAKE_CURRENT_SOURCE_DIR}
  ${CMAKE_CURRENT_SOURCE_DIR}/../llama.cpp/include
  ${CMAKE_CURRENT_SOURCE_DIR}/../llama.cpp/ggml/include
)

target_link_libraries(shard_engine PRIVATE llama)

set_target_properties(shard_engine PROPERTIES
  OUTPUT_NAME "shard_engine"
)
