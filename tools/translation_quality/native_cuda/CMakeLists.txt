cmake_minimum_required(VERSION 3.24)
project(translateit_native_cuda_candidate LANGUAGES C CXX)

# Developer-only candidate, not the Tauri/release build. No download or
# checkout mutation. The caller provides the reviewed patched source tree.
if(NOT WIN32 OR NOT MSVC OR NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    message(FATAL_ERROR "Native candidate requires Windows x64 MSVC")
endif()
set(LLAMA_SOURCE_DIR "" CACHE PATH "Exact patched llama.cpp checkout")
if(NOT IS_ABSOLUTE "${LLAMA_SOURCE_DIR}" OR NOT EXISTS "${LLAMA_SOURCE_DIR}/CMakeLists.txt")
    message(FATAL_ERROR "Provide an absolute llama.cpp source checkout")
endif()
find_package(Git REQUIRED)
execute_process(COMMAND "${GIT_EXECUTABLE}" -C "${LLAMA_SOURCE_DIR}" rev-parse HEAD
    RESULT_VARIABLE git_result OUTPUT_VARIABLE upstream_sha OUTPUT_STRIP_TRAILING_WHITESPACE)
if(NOT git_result EQUAL 0 OR NOT upstream_sha STREQUAL "6d8a8886fa44a4a404c58ace93052ea680193981")
    message(FATAL_ERROR "Unexpected llama.cpp source revision")
endif()
execute_process(COMMAND "${GIT_EXECUTABLE}" -C "${LLAMA_SOURCE_DIR}"
    apply --reverse --check "${CMAKE_CURRENT_SOURCE_DIR}/../llama-strict-cuda.patch"
    RESULT_VARIABLE patch_result)
if(NOT patch_result EQUAL 0)
    message(FATAL_ERROR "Required strict CUDA patch is not present")
endif()
# The reverse check establishes patch presence, not a clean checkout.
# The integrated proof must separately record/review the full source diff.
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(LLAMA_USE_SYSTEM_GGML OFF CACHE BOOL "" FORCE)
set(GGML_BACKEND_DL OFF CACHE BOOL "" FORCE)
set(GGML_CUDA ON CACHE BOOL "" FORCE)
set(GGML_TRANSLATEIT_STRICT_CUDA ON CACHE BOOL "" FORCE)
set(CMAKE_CUDA_ARCHITECTURES 86 CACHE STRING "" FORCE)
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
set(GGML_CPU ON CACHE BOOL "" FORCE)
set(GGML_CUDA_NCCL OFF CACHE BOOL "" FORCE)
set(GGML_CUDA_CCCL_VERSION "" CACHE STRING "" FORCE)
foreach(option IN ITEMS LLAMA_BUILD_COMMON LLAMA_BUILD_TESTS LLAMA_BUILD_EXAMPLES
        LLAMA_BUILD_TOOLS LLAMA_BUILD_SERVER LLAMA_BUILD_APP LLAMA_BUILD_UI
        LLAMA_OPENSSL LLAMA_SUBPROCESS LLAMA_CONNECT MTMD_VIDEO
        GGML_VULKAN GGML_HIP GGML_SYCL GGML_OPENCL GGML_METAL GGML_RPC
        GGML_MUSA GGML_WEBGPU GGML_ZDNN GGML_VIRTGPU GGML_VIRTGPU_BACKEND
        GGML_OPENVINO GGML_ET GGML_HEXAGON GGML_ZENDNN GGML_BLAS
        GGML_OPENMP_FETCH GGML_BUILD_TESTS GGML_BUILD_EXAMPLES)
    set(${option} OFF CACHE BOOL "" FORCE)
endforeach()
set(LLAMA_BUILD_MTMD ON CACHE BOOL "" FORCE)
add_subdirectory("${LLAMA_SOURCE_DIR}" llama-build EXCLUDE_FROM_ALL)

# A single DLL with one statically linked GGML/llama/mtmd closure.
# Only the small candidate C ABI is exported; Rust never mirrors the
# changing upstream by-value parameter structs.
add_library(translateit_native_cuda SHARED asr.cpp)
target_link_libraries(translateit_native_cuda PRIVATE llama mtmd ggml)
target_compile_features(translateit_native_cuda PRIVATE cxx_std_17)
