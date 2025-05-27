# Master CMake configuration to include shaderc and its dependency projects.
# In this way we can track all projects under the same directory, instead
# of using the directory hierarchy required by shaderc, which Git submodule
# does not like.

cmake_minimum_required(VERSION 3.14)

project(native-shaderc)

add_subdirectory(spirv-headers)
add_subdirectory(spirv-tools)
add_subdirectory(glslang)
add_subdirectory(shaderc)
