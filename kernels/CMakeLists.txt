cmake_minimum_required(VERSION 3.28)
project(glmaxx_sm120 LANGUAGES CXX CUDA)

if(NOT DEFINED CUTLASS_DIR)
  message(FATAL_ERROR "CUTLASS_DIR must point to pinned CUTLASS commit e05f953a5b3d38adc240df2ff928e0421c2abba3")
endif()

execute_process(
  COMMAND git -C "${CUTLASS_DIR}" rev-parse HEAD
  OUTPUT_VARIABLE CUTLASS_HEAD
  OUTPUT_STRIP_TRAILING_WHITESPACE
  COMMAND_ERROR_IS_FATAL ANY)
if(NOT CUTLASS_HEAD STREQUAL "e05f953a5b3d38adc240df2ff928e0421c2abba3")
  message(FATAL_ERROR "CUTLASS revision mismatch: ${CUTLASS_HEAD}")
endif()

add_library(glmaxx_sm120 SHARED
  sm120/nvfp4_routed_fc1.cu
  sm120/cutlass_nvfp4_dense_control.cu)
target_include_directories(glmaxx_sm120
  PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include"
  PRIVATE "${CUTLASS_DIR}/include" "${CUTLASS_DIR}/tools/util/include")
target_compile_features(glmaxx_sm120 PRIVATE cxx_std_17 cuda_std_17)
target_compile_options(glmaxx_sm120 PRIVATE
  $<$<COMPILE_LANGUAGE:CUDA>:--threads=0;--expt-relaxed-constexpr;--expt-extended-lambda;-lineinfo;-arch=sm_120f>)
set_target_properties(glmaxx_sm120 PROPERTIES
  CUDA_ARCHITECTURES OFF
  CUDA_SEPARABLE_COMPILATION OFF
  POSITION_INDEPENDENT_CODE ON)

add_executable(glmaxx_cutlass_layout_probe tools/cutlass_layout_probe.cu)
target_include_directories(glmaxx_cutlass_layout_probe PRIVATE
  "${CUTLASS_DIR}/include" "${CUTLASS_DIR}/tools/util/include")
target_compile_features(glmaxx_cutlass_layout_probe PRIVATE cxx_std_17 cuda_std_17)
target_compile_options(glmaxx_cutlass_layout_probe PRIVATE
  $<$<COMPILE_LANGUAGE:CUDA>:-arch=sm_120f>)
set_target_properties(glmaxx_cutlass_layout_probe PROPERTIES
  CUDA_ARCHITECTURES OFF)

add_executable(glmaxx_cutlass_activation_layout_probe
  tools/cutlass_activation_layout_probe.cu)
target_include_directories(glmaxx_cutlass_activation_layout_probe PRIVATE
  "${CUTLASS_DIR}/include" "${CUTLASS_DIR}/tools/util/include")
target_compile_features(glmaxx_cutlass_activation_layout_probe
  PRIVATE cxx_std_17 cuda_std_17)
target_compile_options(glmaxx_cutlass_activation_layout_probe PRIVATE
  $<$<COMPILE_LANGUAGE:CUDA>:-arch=sm_120f>)
set_target_properties(glmaxx_cutlass_activation_layout_probe PROPERTIES
  CUDA_ARCHITECTURES OFF)

# Compile the pinned CUTLASS SM120 dense NVFP4 example as an unlaunched
# tensor-core control. The glmaxx production successor will consume the same
# operand/layout types but needs a GLM-specific paired gate/up epilogue.
add_executable(glmaxx_cutlass_nvfp4_dense_control
  "${CUTLASS_DIR}/examples/79_blackwell_geforce_gemm/79a_blackwell_geforce_nvfp4_bf16_gemm.cu")
target_include_directories(glmaxx_cutlass_nvfp4_dense_control PRIVATE
  "${CUTLASS_DIR}/include"
  "${CUTLASS_DIR}/tools/util/include"
  "${CUTLASS_DIR}/examples/common"
  "${CUTLASS_DIR}/examples/79_blackwell_geforce_gemm")
target_compile_features(glmaxx_cutlass_nvfp4_dense_control
  PRIVATE cxx_std_17 cuda_std_17)
target_compile_options(glmaxx_cutlass_nvfp4_dense_control PRIVATE
  $<$<COMPILE_LANGUAGE:CUDA>:--expt-relaxed-constexpr;--expt-extended-lambda;-arch=sm_120f>)
set_target_properties(glmaxx_cutlass_nvfp4_dense_control PROPERTIES
  CUDA_ARCHITECTURES OFF)
