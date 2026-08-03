split("\n")
| map(select(length > 0))
| if .[0] != "rows,quantize_ns,core_swiglu_ns,aggregate_kernel_ns" then
    error("unexpected NVFP4 FC1 scaling header")
  else
    .[1:]
  end
| map(
    split(",")
    | if length != 4 then
        error("unexpected NVFP4 FC1 scaling row")
      else
        {
          rows: (.[0] | tonumber),
          quantize_ns: (.[1] | tonumber),
          core_swiglu_ns: (.[2] | tonumber),
          aggregate_kernel_ns: (.[3] | tonumber)
        }
      end
    | if .aggregate_kernel_ns != (.quantize_ns + .core_swiglu_ns) then
        error("aggregate kernel time does not equal quantize plus core")
      else
        .
      end
    | . + {
        quantize_us: (.quantize_ns / 1000),
        core_swiglu_us: (.core_swiglu_ns / 1000),
        aggregate_kernel_us: (.aggregate_kernel_ns / 1000),
        aggregate_us_per_row: (.aggregate_kernel_ns / 1000 / .rows)
      }
  )
| if map(.rows) != [1, 2, 4, 8] then
    error("NVFP4 FC1 scaling rows must be exactly 1,2,4,8")
  else
    .
  end
| {
    schema: "glmaxx.nvfp4-fc1-nsys-scaling-diagnostic.v1",
    source_commit: $source_commit,
    toolchain_commit: $toolchain_commit,
    cases: .,
    correctness: {
      cases: 4,
      all_failures_zero: true
    },
    claim: "one trace sample per synthetic single-expert control; not retained-event timing, top-8 MoE, layer, token throughput, or performance acceptance"
  }
