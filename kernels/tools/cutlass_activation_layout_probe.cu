#include <cutlass/detail/sm100_blockscaled_layout.hpp>

#include <cute/tensor.hpp>

#include <array>
#include <cstdint>
#include <cstdio>

namespace {

constexpr uint32_t kN = 1024;
constexpr uint32_t kK = 6144;
constexpr uint32_t kGroupsK = kK / 16;

constexpr uint32_t round_up_128(uint32_t value) {
  return (value + 127u) & ~127u;
}

uint64_t frozen_offset(uint32_t row, uint32_t group) {
  const uint32_t row_block = row / 128;
  const uint32_t row0 = row % 32;
  const uint32_t row1 = (row % 128) / 32;
  const uint32_t k_block = group / 4;
  const uint32_t group_in = group % 4;
  constexpr uint32_t kKBlocks = kK / 64;
  return uint64_t{512} * (uint64_t{row_block} * kKBlocks + k_block) +
         16 * row0 + 4 * row1 + group_in;
}

}  // namespace

int main() {
  using Config = cutlass::detail::Sm1xxBlockScaledConfig<16>;
  constexpr std::array<uint32_t, 17> assignments = {
      1,    2,    4,    8,     16,    32,    64,    127, 128,
      129,  256,  512,  1024,  2048,  8192,  32768, 65535,
  };
  uint64_t comparisons = 0;
  for (const uint32_t rows : assignments) {
    const auto layout = Config::tile_atom_to_shape_SFA(
        cute::make_shape(static_cast<int>(rows), static_cast<int>(kN),
                         static_cast<int>(kK), 1));
    const uint64_t expected_storage =
        uint64_t{round_up_128(rows)} * kGroupsK;
    const uint64_t cutlass_storage =
        static_cast<uint64_t>(cute::cosize(layout));
    if (cutlass_storage != expected_storage) {
      std::fprintf(stderr,
                   "SFA storage mismatch rows=%u cutlass=%llu frozen=%llu\n",
                   rows, static_cast<unsigned long long>(cutlass_storage),
                   static_cast<unsigned long long>(expected_storage));
      return 1;
    }
    for (uint32_t row = 0; row < rows; ++row) {
      for (uint32_t group = 0; group < kGroupsK; ++group) {
        const auto cutlass_offset =
            static_cast<uint64_t>(layout(row, group * 16, 0));
        const auto expected = frozen_offset(row, group);
        if (cutlass_offset != expected) {
          std::fprintf(
              stderr,
              "SFA layout mismatch rows=%u row=%u group=%u cutlass=%llu "
              "frozen=%llu\n",
              rows, row, group,
              static_cast<unsigned long long>(cutlass_offset),
              static_cast<unsigned long long>(expected));
          return 1;
        }
        ++comparisons;
      }
    }
  }
  std::printf("CUTLASS_SFA_LAYOUT_PASS cases=%zu comparisons=%llu\n",
              assignments.size(),
              static_cast<unsigned long long>(comparisons));
  return 0;
}
