#include <cutlass/detail/sm100_blockscaled_layout.hpp>

#include <cute/tensor.hpp>

#include <cstdint>
#include <cstdio>

namespace {

uint64_t frozen_offset(uint32_t n, uint32_t group, uint32_t padded_k) {
  const uint32_t n_block = n / 128;
  const uint32_t n0 = n % 32;
  const uint32_t n1 = (n % 128) / 32;
  const uint32_t k_block = group / 4;
  const uint32_t group_in = group % 4;
  const uint32_t k_blocks = padded_k / 64;
  return uint64_t{512} * (uint64_t{n_block} * k_blocks + k_block) +
         16 * n0 + 4 * n1 + group_in;
}
}  // namespace

int main() {
  using Config = cutlass::detail::Sm1xxBlockScaledConfig<16>;
  constexpr int m = 1;
  constexpr int n = 1024;
  constexpr int k = 6144;
  const auto layout =
      Config::tile_atom_to_shape_SFB(cute::make_shape(m, n, k, 1));
  uint64_t comparisons = 0;
  for (int row = 0; row < n; ++row) {
    for (int group = 0; group < k / 16; ++group) {
      const auto cutlass_offset =
          static_cast<uint64_t>(layout(row, group * 16, 0));
      const auto expected = frozen_offset(row, group, k);
      if (cutlass_offset != expected) {
        std::fprintf(stderr,
                     "layout mismatch row=%d group=%d cutlass=%llu frozen=%llu\n",
                     row, group,
                     static_cast<unsigned long long>(cutlass_offset),
                     static_cast<unsigned long long>(expected));
        return 1;
      }
      ++comparisons;
    }
  }
  std::printf("CUTLASS_SFB_LAYOUT_PASS comparisons=%llu\n",
              static_cast<unsigned long long>(comparisons));
  return 0;
}
