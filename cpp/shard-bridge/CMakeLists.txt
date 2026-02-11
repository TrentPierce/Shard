cmake_minimum_required(VERSION 3.18)
project(shard_engine LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

add_library(shard_engine SHARED
  shard_bridge.cpp
)

target_compile_definitions(shard_engine PRIVATE SHARD_ENGINE_BUILD)

target_include_directories(shard_engine PUBLIC
  ${CMAKE_CURRENT_SOURCE_DIR}
)

set_target_properties(shard_engine PROPERTIES
  OUTPUT_NAME "shard_engine"
)
