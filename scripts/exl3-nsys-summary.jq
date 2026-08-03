split("\n")
| map(select(length > 0))
| if .[0] != "projection,rows,instances,projection_avg_ns,projection_min_ns,projection_max_ns,input_rotation_avg_ns,output_rotation_avg_ns" then
    error("unexpected EXL3 scaling header")
  else
    .[1:]
  end
| map(
    split(",")
    | if length != 8 then
        error("unexpected EXL3 scaling row")
      else
        {
          projection: .[0],
          rows: (.[1] | tonumber),
          instances: (.[2] | tonumber),
          projection_avg_ns: (.[3] | tonumber),
          projection_min_ns: (.[4] | tonumber),
          projection_max_ns: (.[5] | tonumber),
          input_rotation_avg_ns: (.[6] | tonumber),
          output_rotation_avg_ns: (.[7] | tonumber)
        }
      end
    | . + {
        projection_avg_us: (.projection_avg_ns / 1000),
        rotation_avg_us: ((.input_rotation_avg_ns + .output_rotation_avg_ns) / 1000),
        pipeline_avg_us: ((.input_rotation_avg_ns + .projection_avg_ns + .output_rotation_avg_ns) / 1000),
        pipeline_avg_us_per_row: ((.input_rotation_avg_ns + .projection_avg_ns + .output_rotation_avg_ns) / 1000 / .rows)
      }
  )
| sort_by(.rows, .projection)
| if length != 12
    or (map(.rows) | unique) != [1, 2, 4, 8]
    or (map(.projection) | unique) != ["down", "gate", "up"]
    or any(.[]; .instances != 2)
  then
    error("EXL3 scaling matrix must contain two repetitions of all twelve cases")
  else
    .
  end
| {
    schema: "glmaxx.exl3-nsys-scaling-diagnostic.v1",
    source_commit: $source_commit,
    toolchain_commit: $toolchain_commit,
    cases: .,
    correctness: {
      cases: 12,
      all_failures_zero: true,
      all_cpu_gpu_hashes_equal: true,
      all_repeat_bitwise_deterministic: true
    },
    claim: "two traced repetitions per synthetic K=3 control; not retained-event timing, real payload, TP4, layer, token throughput, or performance acceptance"
  }
